# Common mock service for NATS authentication setup
# This provides the same functionality as holo-host-private's holo-nats-auth-setup
# but is self-contained for testing purposes.

{ config, lib, pkgs, ... }:

let
  # Default configuration values
  natsServerHost = config.holo.nats-server.server.host or "0.0.0.0";
  natsListeningPort = config.holo.nats-server.server.port or 4222;
  localCredsPath = config.holo.nats-server.nsc.localCredsPath or "/var/lib/nats_server/local-creds";
  sharedCredsPath = config.holo.nats-server.nsc.sharedCredsPath or "/var/lib/nats_server/shared-creds";
  resolverPath = config.holo.nats-server.nsc.resolverPath or "/var/lib/nats_server/main-resolver.conf";
  nscPath = config.holo.nats-server.nsc.path or "/var/lib/nats-server/nsc/local";
in {
  # Create the mock NATS auth setup service
  systemd.services.holo-nats-auth-setup = {
    description = "NATS JWT Authentication Setup (Mock for Testing)";
    wantedBy = [ "nats.service" ];
    before = [ "nats.service" ];
    
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      User = "root";
      Group = "root";
      TimeoutStartSec = "300";  # 5 minutes timeout
      Environment = [
        "PATH=${pkgs.lib.makeBinPath [ pkgs.nsc pkgs.jq pkgs.openssl ]}:/run/current-system/sw/bin"
      ];
      path = [
        pkgs.nsc
        pkgs.jq
        pkgs.openssl
        pkgs.bash
      ];
    };
    
    script = ''
      #!/usr/bin/env bash
      set -euo pipefail
      
      # NATS JWT Authentication Setup (Mock for Testing)
      # This provides the same functionality as holo-host-private's holo-nats-auth-setup
      # but is self-contained for testing purposes.

      # Function to extract signing keys (from credential_utils.sh)
      extract_signing_key() {
          if [[ -z "$1" ]]; then
              echo "extract_signing_key: name argument is empty, skipping"
              return
          fi
          if [[ -z "$2" ]]; then
              echo "extract_signing_key: sk argument is empty for $1, skipping"
              return
          fi
          local name="$1"
          local sk="$2"
          local nsc_path="$3"
          local local_creds_path="$4"

          if [[ -f "$local_creds_path/$1_SK.nk" ]]; then
              echo "$local_creds_path/$1_SK.nk file already exists."
          else
              local seed_file_path="$nsc_path/keys/keys/${sk:1:1}/${sk:2:2}/$sk.nk"

              echo "Copying file from '$seed_file_path' to '$local_creds_path/$1_SK.nk'"
              cp "$seed_file_path" "$local_creds_path/$1_SK.nk" 2>/dev/null || echo "Warning: Could not copy $1 signing key"
          fi
      }

      # Configuration
      NATS_SERVER_HOST="${natsServerHost}"
      NATS_LISTENING_PORT="${builtins.toString natsListeningPort}"
      LOCAL_CREDS_PATH="${localCredsPath}"
      SHARED_CREDS_PATH="${sharedCredsPath}"
      RESOLVER_PATH="${resolverPath}"
      NSC_PATH="${nscPath}"
      ADMIN_ROLE_NAME="admin_role"
      WORKLOAD_ROLE_NAME="workload_role"
      ORCHESTRATOR_AUTH_USER="orchestrator_auth"

      echo "Setting up NATS JWT authentication (mock service)..."
      echo "Configuration:"
      echo "  NATS_SERVER_HOST: $NATS_SERVER_HOST"
      echo "  NATS_LISTENING_PORT: $NATS_LISTENING_PORT"
      echo "  LOCAL_CREDS_PATH: $LOCAL_CREDS_PATH"
      echo "  SHARED_CREDS_PATH: $SHARED_CREDS_PATH"
      echo "  RESOLVER_PATH: $RESOLVER_PATH"
      echo "  NSC_PATH: $NSC_PATH"

      # Create directories
      mkdir -p "$LOCAL_CREDS_PATH"
      mkdir -p "$SHARED_CREDS_PATH"
      mkdir -p "$NSC_PATH"

      # Create HOLO operator
      nsc add operator --name HOLO --sys --generate-signing-key
      echo "=== OPERATOR CREATED ==="
      nsc describe operator
      echo "=== END OPERATOR DEBUG ==="
      
      nsc edit operator --require-signing-keys \
        --account-jwt-server-url "nats://$NATS_SERVER_HOST:$NATS_LISTENING_PORT" \
        --service-url "nats://$NATS_SERVER_HOST:$NATS_LISTENING_PORT"
      nsc edit operator --sk generate
      
      echo "=== VERIFYING OPERATOR ==="
      nsc describe operator
      echo "=== END OPERATOR DEBUG ==="
      
      # Verify SYS account exists
      echo "=== VERIFYING SYS ACCOUNT ==="
      nsc describe account SYS
      echo "=== END SYS ACCOUNT DEBUG ==="
      
      # Create ADMIN account and signing key
      nsc add account --name ADMIN
      nsc edit account --name ADMIN \
        --js-streams -1 --js-consumer -1 \
        --js-mem-storage 1G --js-disk-storage 5G \
        --conns -1 --leaf-conns -1
      ADMIN_SK="$(echo "$(nsc edit account -n ADMIN --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
      echo "ADMIN_SK: $ADMIN_SK"
      echo "ADMIN_ROLE_NAME: $ADMIN_ROLE_NAME"
      nsc edit signing-key --sk $ADMIN_SK --role $ADMIN_ROLE_NAME \
        --allow-pub "\$JS.>","\$SYS.>","\$G.>","ADMIN.>","AUTH.>","WORKLOAD.>","_INBOX.>","_HPOS_INBOX.>","_ADMIN_INBOX.>","_AUTH_INBOX.>","INVENTORY.>" \
        --allow-sub "\$JS.>","\$SYS.>","\$G.>","ADMIN.>","AUTH.>","WORKLOAD.>","INVENTORY.>","_ADMIN_INBOX.orchestrator.>","_AUTH_INBOX.orchestrator.>" \
        --allow-pub-response

      # Create AUTH account
      nsc add account --name AUTH
      nsc edit account --name AUTH \
        --sk generate --js-streams -1 --js-consumer -1 \
        --js-mem-storage 1G --js-disk-storage 5G \
        --conns -1 --leaf-conns -1
      AUTH_ACCOUNT_PUBKEY=$(nsc describe account AUTH --field sub | jq -r)
      echo "AUTH_ACCOUNT_PUBKEY: $AUTH_ACCOUNT_PUBKEY"
      
      # Extract AUTH signing key pubkey
      AUTH_SK_ACCOUNT_PUBKEY=$(nsc describe account AUTH --field 'nats.signing_keys[0]' | tr -d '"')
      echo "AUTH_SK_ACCOUNT_PUBKEY: $AUTH_SK_ACCOUNT_PUBKEY"
      # AUTH_SK_ACCOUNT_PUBKEY=$(nsc describe account AUTH --field nats.signing_keys | jq -r '.[0].key')
      # 
      # Try to extract AUTH signing key, but handle the case where it might not be available
      # AUTH_SK_ACCOUNT_PUBKEY=""
      # AUTH_SK_OUTPUT=$(nsc describe account AUTH --field nats.signing_keys 2>/dev/null || echo "")
      # if [[ -n "$AUTH_SK_OUTPUT" ]] && [[ "$AUTH_SK_OUTPUT" != "null" ]]; then
      #   AUTH_SK_ACCOUNT_PUBKEY=$(echo "$AUTH_SK_OUTPUT" | jq -r '.[0].key' 2>/dev/null || echo "")
      # fi

      # Create HPOS account
      nsc add account --name HPOS
      nsc edit account --name HPOS \
        --js-streams -1 --js-consumer -1 \
        --js-mem-storage 1G --js-disk-storage 5G \
        --conns -1 --leaf-conns -1
      HPOS_WORKLOAD_SK="$(echo "$(nsc edit account -n HPOS --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
      echo "HPOS_WORKLOAD_SK: $HPOS_WORKLOAD_SK"
      echo "WORKLOAD_ROLE_NAME: $WORKLOAD_ROLE_NAME"
      nsc edit signing-key --sk $HPOS_WORKLOAD_SK --role $WORKLOAD_ROLE_NAME \
        --allow-pub "WORKLOAD.orchestrator.>","WORKLOAD.{{tag(hostId)}}.>","INVENTORY.{{tag(hostId)}}.>","\$JS.API.>","_HPOS_INBOX.{{tag(hostId)}}.>","_ADMIN_INBOX.orchestrator.>" \
        --allow-sub "WORKLOAD.{{tag(hostId)}}.>","INVENTORY.{{tag(hostId)}}.>","\$JS.API.>","_HPOS_INBOX.{{tag(hostId)}}.>" \
        --allow-pub-response

      # Setup export/import rules
      echo "=== Setting up export/import rules ==="
      nsc add export --name ADMIN_WORKLOAD_SERVICE --subject "WORKLOAD.>" --account ADMIN
      echo "=== ADMIN Workload Service export added ==="
      nsc add import --src-account ADMIN --name WORKLOAD_SERVICE --remote-subject "WORKLOAD.>" --local-subject "WORKLOAD.>" --account HPOS
      echo "=== HPOS Workload Service import added ==="
      nsc add export --name HPOS_WORKLOAD_SERVICE --subject "WORKLOAD.>" --account HPOS
      echo "=== HPOS Workload Service export added ==="
      # nsc add import --src-account HPOS --name WORKLOAD_SERVICE --remote-subject "WORKLOAD.>" --local-subject "WORKLOAD.>" --account ADMIN
      # echo "=== ADMIN Workload Service import added ==="

      # Create users
      echo "=== Creating users ==="
      # IMPORTANT: Do not set any explicit permissions for the admin user; permissions are set by the signing key scope only
      echo "Creating Admin user with signing key role scoped permissions"
      nsc add user --name admin_user --account ADMIN -K $ADMIN_ROLE_NAME
      echo "=== Admin user created with sk scoped permissions ==="
      
      # Debug: Check Admin user immediately after creation
      echo "=== DEBUG: ADMIN USER IMMEDIATELY AFTER CREATION ==="
      nsc describe user --name admin_user --account ADMIN
      echo "=== END ADMIN USER DEBUG ==="
      
      # Debug: Check Admin user's signing key assignment
      echo "=== DEBUG: ADMIN USER SIGNING KEY ASSIGNMENT ==="
      nsc describe user --name admin_user --account ADMIN --json | jq '.nats.signing_key'
      echo "=== END ADMIN USER SIGNING KEY ASSIGNMENT ==="
      
      # Debug: Check Admin user's permissions
      echo "=== DEBUG: ADMIN USER PERMISSIONS ==="
      nsc describe user --name admin_user --account ADMIN --json | jq '.nats.permissions'
      echo "=== END ADMIN USER PERMISSIONS DEBUG ==="
      
      echo "Creating Orchestrator Auth user with allow-pubsub permissions"
      nsc add user --name $ORCHESTRATOR_AUTH_USER --account AUTH --allow-pubsub ">"
      echo "=== Orchestrator Auth user created ==="
      
      # Get orchestrator user pubkey after creation
      ORCHESTRATOR_AUTH_USER_PUBKEY=$(nsc describe user --name $ORCHESTRATOR_AUTH_USER --account AUTH --field sub | jq -r)
      echo "ORCHESTRATOR_AUTH_USER_PUBKEY: $ORCHESTRATOR_AUTH_USER_PUBKEY"

      echo "Creating Auth Guard user with deny-pubsub permissions"
      nsc add user --name auth_guard_user --account AUTH --deny-pubsub ">"
      echo "=== Auth Guard user created ==="

      # Debug: Configure Auth Callout
      if nsc describe account AUTH --field nats.authorization 2>/dev/null; then
        echo "AUTH account already has the auth callout set."
      else
        nsc edit authcallout --account AUTH --allowed-account "\"$AUTH_ACCOUNT_PUBKEY\",\"$AUTH_SK_ACCOUNT_PUBKEY\"" --auth-user $ORCHESTRATOR_AUTH_USER_PUBKEY
      fi

      # Extract signing keys
      echo "=== Extracting signing keys ==="
      LOCAL_CREDS_DIR="$LOCAL_CREDS_PATH"

      # Debug: Show variable values
      echo "DEBUG: ADMIN_SK='$ADMIN_SK'"
      echo "DEBUG: AUTH_SK_ACCOUNT_PUBKEY='$AUTH_SK_ACCOUNT_PUBKEY'"
      echo "DEBUG: AUTH_ACCOUNT_PUBKEY='$AUTH_ACCOUNT_PUBKEY'"
      echo "DEBUG: NSC_PATH='$NSC_PATH'"
      echo "DEBUG: LOCAL_CREDS_DIR='$LOCAL_CREDS_DIR'"

      # Extract ADMIN signing key
      echo "=== Extracting ADMIN signing key ==="
      if [[ -n "$ADMIN_SK" ]]; then
        echo "Extracting ADMIN signing key: $ADMIN_SK"
        echo "Calling extract_signing_key ADMIN '$ADMIN_SK' '$NSC_PATH' '$LOCAL_CREDS_DIR'"
        extract_signing_key ADMIN "$ADMIN_SK" "$NSC_PATH" "$LOCAL_CREDS_DIR"
      else
        echo "Warning: ADMIN_SK is empty"
      fi

      # Extract AUTH signing key
      echo "=== Extracting AUTH signing key ==="
      if [[ -n "$AUTH_SK_ACCOUNT_PUBKEY" ]]; then
        echo "Extracting AUTH signing key: $AUTH_SK_ACCOUNT_PUBKEY"
        echo "Calling extract_signing_key AUTH '$AUTH_SK_ACCOUNT_PUBKEY' '$NSC_PATH' '$LOCAL_CREDS_DIR'"
        extract_signing_key AUTH "$AUTH_SK_ACCOUNT_PUBKEY" "$NSC_PATH" "$LOCAL_CREDS_DIR"
      else
        echo "Warning: AUTH_SK_ACCOUNT_PUBKEY is empty"
      fi

      # Extract AUTH_ROOT signing key
      echo "=== Extracting AUTH_ROOT signing key ==="
      if [[ -n "$AUTH_ACCOUNT_PUBKEY" ]]; then
        echo "Extracting AUTH_ROOT signing key: $AUTH_ACCOUNT_PUBKEY"
        echo "Calling extract_signing_key AUTH_ROOT '$AUTH_ACCOUNT_PUBKEY' '$NSC_PATH' '$LOCAL_CREDS_DIR'"
        extract_signing_key AUTH_ROOT "$AUTH_ACCOUNT_PUBKEY" "$NSC_PATH" "$LOCAL_CREDS_DIR"
      else
        echo "Warning: AUTH_ACCOUNT_PUBKEY is empty"
      fi

      # Generate shared credentials and JWTs
      nsc describe operator --raw --output-file "$SHARED_CREDS_PATH/HOLO.jwt"
      
      # Create SYS account JWT with correct naming (just account ID)
      SYS_ACCOUNT_ID=$(nsc describe account SYS --field sub | jq -r)
      echo "SYS_ACCOUNT_ID: $SYS_ACCOUNT_ID"
      nsc describe account --name SYS --raw --output-file "$SHARED_CREDS_PATH/$SYS_ACCOUNT_ID.jwt"
      
      # Create ADMIN Account JWT with correct naming (just account ID)
      ADMIN_ACCOUNT_ID=$(nsc describe account ADMIN --field sub | jq -r)
      echo "ADMIN_ACCOUNT_ID: $ADMIN_ACCOUNT_ID"
      nsc describe account --name ADMIN --raw --output-file "$SHARED_CREDS_PATH/$ADMIN_ACCOUNT_ID.jwt"
      
      # Create AUTH Account JWT with correct naming (just account ID)
      AUTH_ACCOUNT_ID=$(nsc describe account AUTH --field sub | jq -r)
      echo "AUTH_ACCOUNT_ID: $AUTH_ACCOUNT_ID"
      nsc describe account --name AUTH --raw --output-file "$SHARED_CREDS_PATH/$AUTH_ACCOUNT_ID.jwt"
      
      # Create HPOS Account JWT with correct naming (just account ID)
      HPOS_ACCOUNT_ID=$(nsc describe account HPOS --field sub | jq -r)
      echo "HPOS_ACCOUNT_ID: $HPOS_ACCOUNT_ID"
      nsc describe account --name HPOS --raw --output-file "$SHARED_CREDS_PATH/$HPOS_ACCOUNT_ID.jwt"
      
      # Debug: List all JWT files created
      echo "=== JWT files created ==="
      ls -la "$SHARED_CREDS_PATH/"*.jwt
      echo "=== End JWT files ==="

      nsc generate creds --name auth_guard_user --account AUTH --output-file "$SHARED_CREDS_PATH/auth_guard_user.creds"
      nsc generate creds --name admin_user --account ADMIN --output-file "$SHARED_CREDS_PATH/admin_user.creds"
      nsc generate creds --name $ORCHESTRATOR_AUTH_USER --account AUTH --output-file "$SHARED_CREDS_PATH/$ORCHESTRATOR_AUTH_USER.creds"

      # Debug: Check admin user permissions
      echo "=== ADMIN USER JWT ==="
      nsc describe user --name admin_user --account ADMIN
      echo "=== ADMIN USER CREDS ==="
      cat "$SHARED_CREDS_PATH/admin_user.creds" | head -20
      echo "=== END ADMIN DEBUG ==="

      echo "=== ADMIN USER JWT PERMISSIONS ==="
      nsc describe user --name admin_user --account ADMIN --json | jq '.nats.permissions'
      echo "=== END ADMIN USER JWT PERMISSIONS ==="

      # Debug: Check admin user's signing key assignment
      echo "=== ADMIN USER SIGNING KEY ASSIGNMENT ==="
      nsc describe user --name admin_user --account ADMIN --json | jq '.nats.signing_key'
      echo "=== END ADMIN USER SIGNING KEY ASSIGNMENT ==="

      # Debug: Check creds file contents in detail
      echo "=== ADMIN USER CREDS FILE FULL CONTENTS ==="
      cat "$SHARED_CREDS_PATH/admin_user.creds"
      echo "=== END ADMIN USER CREDS FILE ==="

      # Debug: Check if admin user can connect and has permissions
      echo "=== TESTING ADMIN USER CONNECTION ==="
      timeout 10 nats --creds "$SHARED_CREDS_PATH/admin_user.creds" --server "nats://localhost:$NATS_LISTENING_PORT" pub test.admin "test message" || echo "Connection test failed (expected if NATS not running yet)"
      echo "=== END CONNECTION TEST ==="

      # Generate resolver config
      nsc generate config --nats-resolver --sys-account SYS --force --config-file "$RESOLVER_PATH"

      # Set permissions
      chown -R nats-server:nats-server /var/lib/nats_server
      chmod -R 700 "$LOCAL_CREDS_PATH"
      chmod -R 700 "$SHARED_CREDS_PATH"
      chmod 644 "$RESOLVER_PATH"

      # Verify files exist and are accessible
      echo "=== Checking shared creds directory ==="
      ls -la "$SHARED_CREDS_PATH/"
      echo "=== Checking resolver config ==="
      ls -la "$RESOLVER_PATH"
      echo "=== Checking file contents ==="
      cat "$SHARED_CREDS_PATH/HOLO.jwt" | head -1
      echo "=== File check complete ==="

      # Ensure files are fully written
      sync
      sleep 1

      # Double-check the exact path NATS is looking for
      echo "=== NATS expected path ==="
      echo "$SHARED_CREDS_PATH/HOLO.jwt"
      ls -la "$SHARED_CREDS_PATH/HOLO.jwt"
      echo "=== End path check ==="

      # Debug: Check what system account is in the operator JWT
      echo "=== Operator JWT system account ==="
      nsc describe operator --field system_account
      echo "=== End operator JWT check ==="

      echo "NATS JWT authentication setup completed (mock service)"
    '';
  };

  # Ensure NATS service waits for auth setup
  systemd.services.nats.after = [ "network-online.target" "holo-nats-auth-setup.service" ];
  systemd.services.nats.requires = [ "network-online.target" "holo-nats-auth-setup.service" ];
} 
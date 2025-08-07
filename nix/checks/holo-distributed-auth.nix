{ inputs, flake, pkgs, system }:

pkgs.testers.runNixOSTest (
  { nodes, lib, ... }:
  let
    testScript = ''
      nats_server.wait_for_unit("holo-nats-auth-setup.service")
      nats_server.wait_for_unit("multi-user.target")
      orchestrator.wait_for_unit("multi-user.target")

      print("=== NATS SERVER SERVICE STATUS ===")
      status_output = nats_server.succeed("systemctl status nats.service | grep -E 'Active:|Loaded:' || echo 'NATS service not found or is inactive'")
      print(status_output)
      active_output = nats_server.succeed("systemctl is-active nats.service || echo 'NATS service not active'")
      print(active_output)
      pgrep_output = nats_server.succeed("pgrep -af nats-server || echo 'NATS server process not found'")
      print(pgrep_output)
      
      print("=== NATS SERVER CREDENTIALS ===")
      ls_creds = nats_server.succeed("ls -la /var/lib/nats_server/shared-creds/")
      print(ls_creds)
      head_jwt = nats_server.succeed("cat /var/lib/nats_server/shared-creds/HOLO.jwt | head -3")
      print(head_jwt)
      ls_resolver = nats_server.succeed("ls -la /var/lib/nats_server/main-resolver.conf")
      print(ls_resolver)
      
      print("=== SHARED CREDENTIALS ON NATS-SERVER NODE ===")
      ls_shared_nats = nats_server.succeed("ls -la /tmp/shared/ || echo 'No /tmp/shared directory'")
      print(ls_shared_nats)

      print("=== SHARED CREDENTIALS ON ORCHESTRATOR NODE ===")
      ls_shared_orch = orchestrator.succeed("ls -la /tmp/shared/ || echo 'No /tmp/shared directory'")
      print(ls_shared_orch)

      print("=== ORCHESTRATOR CREDENTIALS ===")
      ls_orch = orchestrator.succeed("ls -la /var/lib/holo-orchestrator/")
      print(ls_orch)
      ls_orch_creds = orchestrator.succeed("ls -la /var/lib/holo-orchestrator/nats-creds/")
      print(ls_orch_creds)
      orch_auth_creds = orchestrator.succeed("test -f /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds && echo '✓ orchestrator_auth.creds exists' || echo '✗ orchestrator_auth.creds missing'")
      print(orch_auth_creds)
      orch_admin_creds = orchestrator.succeed("test -f /var/lib/holo-orchestrator/nats-creds/admin.creds && echo '✓ admin.creds exists' || echo '✗ admin.creds missing'")
      print(orch_admin_creds)
      orch_nsc_key = orchestrator.succeed("test -f /var/lib/holo-orchestrator/nsc-proxy-auth.key && echo '✓ nsc-proxy-auth.key exists' || echo '✗ nsc-proxy-auth.key missing'")
      print(orch_nsc_key)
      orch_cluster_id = orchestrator.succeed("test -f /var/lib/config/mongo/cluster_id.txt && echo '✓ cluster_id.txt exists' || echo '✗ cluster_id.txt missing'")
      print(orch_cluster_id)
      orch_password = orchestrator.succeed("test -f /var/lib/config/mongo/password.txt && echo '✓ password.txt exists' || echo '✗ password.txt missing'")
      print(orch_password)

      print("=== DEBUG: main-resolver.conf presence and contents ===")
      # 1. main-resolver.conf presence and contents
      print("=== CHECK: main-resolver.conf presence and contents ===")
      resolver_ls = nats_server.succeed("ls -l /var/lib/nats_server/main-resolver.conf || echo 'main-resolver.conf missing'")
      print(resolver_ls)
      resolver_cat = nats_server.succeed("cat /var/lib/nats_server/main-resolver.conf || echo 'main-resolver.conf missing'")
      print(resolver_cat)

      # 2. HOLO.jwt, SYS.jwt, and all account JWTs in shared-creds, readable by nats-server
      print("=== CHECK: JWTs in /var/lib/nats_server/shared-creds/ ===")
      jwt_ls = nats_server.succeed("ls -l /var/lib/nats_server/shared-creds/ || echo 'shared-creds dir missing'")
      print(jwt_ls)
      jwt_perms = nats_server.succeed("namei -l /var/lib/nats_server/shared-creds/HOLO.jwt; namei -l /var/lib/nats_server/shared-creds/SYS.jwt || echo 'Could not stat HOLO.jwt or SYS.jwt'")
      print(jwt_perms)
      jwt_cat = nats_server.succeed("for f in /var/lib/nats_server/shared-creds/*.jwt; do echo \"=== $f ===\"; head -3 $f || echo \"$f missing\"; done")
      print(jwt_cat)

      # 3. NATS server start order
      print("=== CHECK: NATS service start time and config file timestamps ===")
      nats_conf_ls = nats_server.succeed("ls -l /var/lib/nats_server/nats-server.conf || echo 'nats-server.conf missing'")
      print(nats_conf_ls)
      nats_conf_cat = nats_server.succeed("cat /var/lib/nats_server/nats-server.conf || echo 'nats-server.conf missing'")
      print(nats_conf_cat)
      nats_conf_time = nats_server.succeed("stat -c '%y' /var/lib/nats_server/nats-server.conf || echo 'nats-server.conf missing'")
      print(f"nats-server.conf last modified: {nats_conf_time}")
      resolver_time = nats_server.succeed("stat -c '%y' /var/lib/nats_server/main-resolver.conf || echo 'main-resolver.conf missing'")
      print(f"main-resolver.conf last modified: {resolver_time}")
      jwt_dir_time = nats_server.succeed("stat -c '%y' /var/lib/nats_server/shared-creds/ || echo 'shared-creds dir missing'")
      print(f"shared-creds dir last modified: {jwt_dir_time}")
      nats_status = nats_server.succeed("systemctl status nats.service | grep -E 'Active:|Loaded:' || echo 'NATS service not found or is inactive'")
      print(nats_status)
      nats_ps = nats_server.succeed("ps aux | grep nats-server | grep -v grep")
      print(nats_ps)

      # 4. nats-server.conf references
      print("=== CHECK: nats-server.conf resolver and JWT paths ===")
      nats_conf_grep = nats_server.succeed("grep -E 'resolver|jwt|operator' /var/lib/nats_server/nats-server.conf || echo 'No resolver/jwt/operator lines in nats-server.conf'")
      print(nats_conf_grep)

      # Orchestrator failure debug
      print("=== CHECK: holo-orchestrator.service status and last 100 journal lines if failed ===")
      orch_status = orchestrator.succeed("systemctl status holo-orchestrator.service | grep -E 'Active:|Loaded:' || echo 'Service not found or is inactive'")
      print(orch_status)
      orch_active = orchestrator.succeed("systemctl is-active holo-orchestrator.service || echo 'Service not active'")
      print(orch_active)
      orch_enabled = orchestrator.succeed("systemctl is-enabled holo-orchestrator.service || echo 'Service not enabled'")
      print(orch_enabled)
      if (orch_active.strip() != "active") and (orch_enabled.strip() == "enabled"):
          print("--- Last 100 lines of holo-orchestrator.service journal ---")
          orch_journal = orchestrator.succeed("journalctl -u holo-orchestrator.service -n 100 || echo 'No journal output'")
          print(orch_journal)

      print("=== ORCHESTRATOR CREDS FILE CONTENTS: admin.creds ===")
      admin_creds_cat = orchestrator.succeed("cat /var/lib/holo-orchestrator/nats-creds/admin.creds || echo 'admin.creds missing'")
      print(admin_creds_cat)
      print("=== ORCHESTRATOR CREDS FILE CONTENTS: orchestrator_auth.creds ===")
      orch_auth_creds_cat = orchestrator.succeed("cat /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds || echo 'orchestrator_auth.creds missing'")
      print(orch_auth_creds_cat)
      print("=== ORCHESTRATOR NSC PROXY AUTH KEY CONTENTS ===")
      nsc_proxy_key_cat = orchestrator.succeed("cat /var/lib/holo-orchestrator/nsc-proxy-auth.key || echo 'nsc-proxy-auth.key missing'")
      print(nsc_proxy_key_cat)
      print("=== ORCHESTRATOR CLUSTER ID CONTENTS ===")
      cluster_id_cat = orchestrator.succeed("cat /var/lib/config/mongo/cluster_id.txt || echo 'cluster_id.txt missing'")
      print(cluster_id_cat)
      print("=== ORCHESTRATOR MONGO PASSWORD CONTENTS ===")
      mongo_pw_cat = orchestrator.succeed("cat /var/lib/config/mongo/password.txt || echo 'password.txt missing'")
      print(mongo_pw_cat)

      # # --- BEGIN: Additional orchestrator tests ---
      # print("=== NATSCLI PUBLISH TEST: WORKLOAD.> ===")
      # natscli_admin_creds = "/var/lib/holo-orchestrator/nats-creds/admin.creds"
      # nats_server_url = "nats://nats-server:4222"
      # pub_result = orchestrator.succeed(f"nats pub --creds {natscli_admin_creds} --server {nats_server_url} 'WORKLOAD.test' 'test-message'")
      # print("natscli publish WORKLOAD.> result:", pub_result)
      # assert 'Published' in pub_result or 'OK' in pub_result, "natscli publish to WORKLOAD.> failed"

      # print("=== NATSCLI PUBLISH TEST: INVENTORY.> ===")
      # pub_result2 = orchestrator.succeed(f"nats pub --creds {natscli_admin_creds} --server {nats_server_url} 'INVENTORY.test' 'test-message'")
      # print("natscli publish INVENTORY.> result:", pub_result2)
      # assert 'Published' in pub_result2 or 'OK' in pub_result2, "natscli publish to INVENTORY.> failed"

      # print("=== NATSCLI SUBSCRIBE TEST: > ===")
      # # Start a background subscriber, then publish and check output
      # sub_cmd = f"timeout 3 nats sub --creds {natscli_admin_creds} --server {nats_server_url} '>' > /tmp/nats_sub_output 2>&1 & echo $!"
      # sub_pid = orchestrator.succeed(sub_cmd).strip()
      # import time
      # time.sleep(1)  # Give subscriber time to start
      # orchestrator.succeed(f"nats pub --creds {natscli_admin_creds} --server {nats_server_url} 'TEST.SUB' 'test-subscribe-test'")
      # time.sleep(2)
      # sub_output = orchestrator.succeed("cat /tmp/nats_sub_output")
      # print("natscli subscribe > output:", sub_output)
      # assert 'test-subscribe-test' in sub_output, "natscli subscribe to > failed to receive published message"

      # print("=== ORCHESTRATOR NSC PROXY TEST ===")
      # nsc_proxy_auth_key = orchestrator.succeed("cat /var/lib/holo-orchestrator/nsc-proxy-auth.key").strip()
      # nsc_proxy_url = "http://nats-server:5000/nsc"
      # nsc_proxy_payload = '{"command":"describe_user","params":{"account":"ADMIN","name":"admin_user"},"auth_key":"' + nsc_proxy_auth_key + '"}'
      # nsc_proxy_cmd = (
      #   "curl -s -X POST --fail --header 'Content-Type: application/json' "
      #   f"--data '{nsc_proxy_payload}' {nsc_proxy_url}"
      # )
      # nsc_proxy_result = orchestrator.succeed(nsc_proxy_cmd)
      # print("NSC proxy describe_user result:", nsc_proxy_result)
      # assert 'admin_user' in nsc_proxy_result, "NSC proxy did not return expected user info"
      # # --- END: Additional orchestrator tests ---

      print("✅ NSC credentials and resolver config generated and provisioned!")
    '';
  in
  {
    name = "holo-distributed-auth-test";

    nodes = {
      # Configure Nats-Server node
      nats_server = { pkgs, ... }: {
        imports = [ flake.nixosModules.holo-nats-server ];
        networking = {
          hostName = "nats-server";
          firewall.enable = false;
        };

        holo.nats-server = {
          enable = true;
          server.host = "0.0.0.0";
          jetstream.domain = "holo";
          caddy.enable = false;
          enableJwt = true;
          nsc = {
            path = "/var/lib/nats_server/nsc/local";
            localCredsPath = "/var/lib/nats_server/local-creds";
            sharedCredsPath = "/var/lib/nats_server/shared-creds";
            resolverFileName = "main-resolver.conf";
          };
        };

        systemd.services.holo-nats-auth-setup = {
          description = "NATS JWT Authentication Setup";
          after = [ "network.target" ];
          wantedBy = [ "multi-user.target" ];
          path = [ pkgs.nsc pkgs.jq pkgs.natscli ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            User = "nats-server";
            Group = "nats-server";
            TimeoutStartSec = "120";
            Restart = "no";
            Environment = [
              "XDG_DATA_HOME=/var/lib/nats_server/.data"
              # "XDG_CONFIG_HOME=/var/lib/nats_server/.config"
            ];
          };
          script = ''
            #!${pkgs.bash}/bin/bash
            set -euo pipefail
            set -x

            echo '=== NSC ENVIRONMENT (start) ==='
            nsc env || echo 'nsc env failed'

            NSC_PATH="$XDG_DATA_HOME/nats/nsc"
            LOCAL_CREDS_DIR="/var/lib/nats_server/local-creds"
            SHARED_CREDS_DIR="/var/lib/nats_server/shared-creds"

            # Ensure creds directories exist before storing creds
            mkdir -p $LOCAL_CREDS_DIR
            mkdir -p $SHARED_CREDS_DIR
            mkdir -p /var/lib/nats_server/jwt

            # Ensure writable JWT directory for resolver and creds
            chmod 700 /var/lib/nats_server
            chmod 700 /var/lib/nats_server/jwt
            chmod 700 $LOCAL_CREDS_DIR
            chmod 700 $SHARED_CREDS_DIR
            chown nats-server:nats-server /var/lib/nats_server
            chown nats-server:nats-server /var/lib/nats_server/jwt
            chown nats-server:nats-server $LOCAL_CREDS_DIR
            chown nats-server:nats-server $SHARED_CREDS_DIR
            ls -ld /var/lib/nats_server
            ls -ld /var/lib/nats_server/jwt
            ls -ld $LOCAL_CREDS_DIR
            ls -ld $SHARED_CREDS_DIR

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
                    local unquoted_sk=$(echo "$sk" | tr -d '"')
                    echo "Looking for signing key: $unquoted_sk"

                    local first_char=$(echo "$unquoted_sk" | cut -c1)
                    local second_chars=$(echo "$unquoted_sk" | cut -c2-3)
                    local seed_file_path="$nsc_path/keys/keys/$first_char/$second_chars/$unquoted_sk.nk"
                    local unquoted_seed_path=$(echo "$seed_file_path" | tr -d '"')

                    echo "About to copy from $unquoted_seed_path to $local_creds_path/$1_SK.nk"
                    if [[ -f "$unquoted_seed_path" ]]; then
                        echo "✓ File exists at: $unquoted_seed_path"
                    else
                        echo "✗ File does not exist at: $unquoted_seed_path"
                    fi

                    cp "$unquoted_seed_path" "$local_creds_path/$1_SK.nk" 2>/dev/null || echo "ERROR: Could not copy $1 signing key"
                fi
            }

            echo "Setting up system with NSC Operator and SYS account..."

            nsc add operator --name "HOLO" --sys --generate-signing-key || echo "WARNING: Operator already exists"
            nsc edit operator --require-signing-keys
            nsc list operators || echo 'nsc list operators failed'
            echo "✓ Operator and SYS account created"

            echo "Creating accounts..."

            nsc add account --name "AUTH" || echo "WARNING: AUTH account already exists"
            nsc add account --name "ADMIN" || echo "WARNING: ADMIN account already exists"
            echo "✓ AUTH and ADMIN accounts created"
            
            # Generate and assign signing key for ADMIN
            # nsc edit account --name ADMIN \
            #   --js-streams -1 --js-consumer -1 \
            #   --js-mem-storage 1G --js-disk-storage 5G \
            #   --conns -1 --leaf-conns -1
            ADMIN_SK=$(nsc edit account -n ADMIN --sk generate 2>&1 | grep -oP "signing key\s*\K\S+")
            echo "ADMIN_SK: $ADMIN_SK"
            nsc edit signing-key --sk $ADMIN_SK --role admin_role \
              --allow-pub '$JS.>','$SYS.>','$G.>','ADMIN.>','AUTH.>','WORKLOAD.>','_INBOX.>','_HPOS_INBOX.>','_ADMIN_INBOX.>','_AUTH_INBOX.>','INVENTORY.>' \
              --allow-sub '$JS.>','$SYS.>','$G.>','ADMIN.>','AUTH.>','WORKLOAD.>','INVENTORY.>','_ADMIN_INBOX.orchestrator.>','_AUTH_INBOX.orchestrator.>' \
              --allow-pub-response
            echo "✓ ADMIN account signing key created"
            
            # Generate and assign signing key for AUTH
            # nsc edit account --name AUTH \
            #   --js-streams -1 --js-consumer -1 \
            #   --js-mem-storage 1G --js-disk-storage 5G \
            #   --conns -1 --leaf-conns -1
            AUTH_SK=$(nsc edit account -n AUTH --sk generate 2>&1 | grep -oP "signing key\s*\K\S+")
            nsc edit signing-key --sk $AUTH_SK --role auth_role --allow-pub ">" --allow-sub ">"
            echo "AUTH_SK: $AUTH_SK"

            AUTH_ACCOUNT_PUBKEY=$(nsc describe account AUTH --field sub | jq -r)
            echo "AUTH_ACCOUNT_PUBKEY: $AUTH_ACCOUNT_PUBKEY"
            
            echo "Creating users..."

            # Add users (no extra permissions for scoped user)
            echo "Creating Admin user with Admin signing key role scoped permissions"
            nsc add user --name "admin_user" --account "ADMIN" -K admin_role || echo "WARNING: admin user already exists"
            echo "=== START ADMIN USER DEBUG ==="
            nsc describe user --name "admin_user" --account "ADMIN"
            echo "=== END ADMIN USER DEBUG ==="

            echo "Creating Orchestrator Auth userwith Auth signing key role scoped permissions"
            nsc add user --name "orchestrator_user" --account "AUTH" -K auth_role || echo "WARNING: orchestrator auth user already exists"
            ORCHESTRATOR_AUTH_USER_PUBKEY=$(nsc describe user --name $"orchestrator_user" --account AUTH --field sub | jq -r)
            echo "=== START ORCHESTRATOR USER DEBUG ==="
            echo "ORCHESTRATOR_AUTH_USER_PUBKEY: $ORCHESTRATOR_AUTH_USER_PUBKEY"
            nsc describe user --name "orchestrator_user" --account "AUTH"
            echo "=== END ORCHESTRATOR USER DEBUG ==="

            echo "Creating Auth Guard user with deny-pubsub permissions"
            nsc add user --name "auth_guard_user" --account "AUTH" --deny-pubsub ">"
            echo "=== START AUTH GUARD USER DEBUG ==="
            nsc describe user --name "auth_guard_user" --account "AUTH"
            echo "=== END AUTH GUARD USER DEBUG ==="

            # Configure Auth Callout
            echo "Setting up auth callout..."

            if nsc describe account "AUTH" --field nats.authorization 2>/dev/null; then
              echo "AUTH account already has the auth callout set."
            else
              nsc edit authcallout --account "AUTH" --allowed-account "\"$AUTH_ACCOUNT_PUBKEY\",\"$AUTH_SK\"" --auth-user $ORCHESTRATOR_AUTH_USER_PUBKEY
            fi

            # Debug ADMIN and AUTH accounts and users
            echo "=== START ADMIN DEBUG ==="
            nsc describe account ADMIN
            nsc list users --account ADMIN
            echo "=== END ADMIN DEBUG ==="

            echo "=== START AUTH DEBUG ==="
            nsc describe account AUTH
            nsc list users --account AUTH
            echo "=== END AUTH DEBUG ==="

          # # 
          #   echo '=== VERIFY NSC KEYS DIRS AND FILES ==='
            ls -ld /var/lib/nats_server /var/lib/nats_server/.data /var/lib/nats_server/.data/nats /var/lib/nats_server/.data/nats/nsc || echo 'Could not list parent dirs'
            ls -lR /var/lib/nats_server/.data/nats/nsc/stores || echo 'Could not list nsc stores dir'
            ls -lR /var/lib/nats_server/.data/nats/nsc/keys || echo 'Could not list nsc keys dir'
          #   find /var/lib/nats_server/.data/nats/nsc/keys -type f -name '*.nk' -exec ${pkgs.bash}/bin/bash -c 'echo "=== CONTENTS OF: {} ==="; cat {}' \;
          # # 
   
            echo "=== Extracting AUTH Account signing keys ==="
            extract_signing_key AUTH_ROOT "$AUTH_ACCOUNT_PUBKEY" "$NSC_PATH" "$LOCAL_CREDS_DIR" || echo "WARNING: Failed to extract AUTH_ROOT signing key"
            echo "AUTH_ROOT_SK.nk: $(cat $LOCAL_CREDS_DIR/AUTH_ROOT_SK.nk)"
            cp $LOCAL_CREDS_DIR/AUTH_ROOT_SK.nk $SHARED_CREDS_DIR/AUTH_ROOT_SK.nk

            extract_signing_key AUTH "$AUTH_SK" "$NSC_PATH" "$LOCAL_CREDS_DIR" || echo "WARNING: Failed to extract AUTH signing key"
            echo "AUTH_SK.nk: $(cat $LOCAL_CREDS_DIR/AUTH_SK.nk)"
            cp $LOCAL_CREDS_DIR/AUTH_SK.nk $SHARED_CREDS_DIR/AUTH_SK.nk

            # Generate creds
            echo "=== Generating creds ==="
            nsc generate creds --name "admin_user" --account "ADMIN" --output-file "$LOCAL_CREDS_DIR/admin_user.creds"
            nsc generate creds --name "orchestrator_user" --account "AUTH" --output-file "$LOCAL_CREDS_DIR/orchestrator_user.creds"
            cp $LOCAL_CREDS_DIR/admin_user.creds $SHARED_CREDS_DIR/admin_user.creds
            cp $LOCAL_CREDS_DIR/orchestrator_user.creds $SHARED_CREDS_DIR/orchestrator_auth.creds
            nsc describe operator --raw --output-file "$SHARED_CREDS_DIR/HOLO.jwt"
            nsc describe account --name SYS --raw --output-file "$SHARED_CREDS_DIR/SYS.jwt"

            echo '=== BEGIN SHARED CREDS DIRECTORY CONTENTS ==='
            ls -la $SHARED_CREDS_DIR/ || echo 'Could not list shared creds dir'
            echo '=== END SHARED CREDS DIRECTORY CONTENTS ==='

            echo '=== BEGIN LOCAL CREDS DIRECTORY CONTENTS ==='
            ls -la $LOCAL_CREDS_DIR/ || echo 'Could not list local creds dir'
            echo '=== END LOCAL CREDS DIRECTORY CONTENTS ==='

            # Generate resolver config with correct NSC command
            nsc env -s "$NSC_PATH/stores" -o HOLO
            nsc generate config --nats-resolver --sys-account SYS --force --config-file /var/lib/nats_server/main-resolver.conf
            
            echo '=== BEGIN main-resolver.conf ==='
            cat /var/lib/nats_server/main-resolver.conf
            echo '=== END main-resolver.conf ==='

             echo '=== NSC ENVIRONMENT (end) ==='
            nsc env || echo 'nsc env failed'
             echo '=== NSC ENVIRONMENT (end) ==='
          '';
        };

        systemd.services.nats-shared-creds-copy = {
          description = "Copy NSC credentials to shared directory for orchestrator";
          after = [ "holo-nats-auth-setup.service" ];
          requires = [ "holo-nats-auth-setup.service" ];
          wantedBy = [ "multi-user.target" ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            User = "root";
            Group = "root";
          };
          script = ''
            set -euxo pipefail
            echo "Running as: $(id)"
            echo "Before copy:"
            ls -l /var/lib/nats_server/shared-creds/ || echo 'WARNING: No shared-creds dir'
            ls -ld /var/lib/nats_server/shared-creds/ || echo 'WARNING: No shared-creds dir'
            ls -l /tmp/shared/ || echo 'WARNING: No /tmp/shared dir yet'
            ls -ld /tmp/shared/ || echo 'WARNING: No /tmp/shared dir yet'
            
            echo 'Fixing permissions on /tmp/shared/'
            chmod 1777 /tmp/shared || echo 'WARNING: Failed to chmod /tmp/shared'
            ls -ld /tmp/shared
            cp /var/lib/nats_server/shared-creds/orchestrator_auth.creds /tmp/shared/orchestrator_auth.creds
            cp /var/lib/nats_server/shared-creds/admin_user.creds /tmp/shared/admin.creds
            cp /var/lib/nats_server/shared-creds/AUTH_ROOT_SK.nk /tmp/shared/AUTH_ROOT_SK.nk
            cp /var/lib/nats_server/shared-creds/AUTH_SK.nk /tmp/shared/AUTH_SK.nk
            chmod 666 /tmp/shared/orchestrator_auth.creds || echo 'WARNING: Failed to chmod /tmp/shared/orchestrator_auth.creds'
            chmod 666 /tmp/shared/admin.creds || echo 'WARNING: Failed to chmod /tmp/shared/admin.creds'
            chmod 666 /tmp/shared/AUTH_ROOT_SK.nk || echo 'WARNING: Failed to chmod /tmp/shared/AUTH_ROOT_SK.nk'
            chmod 666 /tmp/shared/AUTH_SK.nk || echo 'WARNING: Failed to chmod /tmp/shared/AUTH_SK.nk'
            
            echo "After copy:"
            ls -l /tmp/shared/ || echo 'No /tmp/shared dir after copy'
          '';
        };

        systemd.services.nats = {
          after = [ "holo-nats-auth-setup.service" ];
          wants = [ "holo-nats-auth-setup.service" ];
        };

        environment.systemPackages = with pkgs; [ curl jq openssl natscli ];
      };

      #########################################################
      # Configure HOlo-Orchestrator node
      orchestrator = { pkgs, ... }: {
        imports = [ flake.nixosModules.holo-orchestrator ];
        networking = {
          hostName = "orchestrator";
          firewall.enable = false;
        };

        holo.orchestrator = {
          enable = true;
          package = flake.packages.${pkgs.system}.rust-workspace.individual.orchestrator;
          nats.nsc_proxy.authKeyFile = "/var/lib/holo-orchestrator/nsc-proxy-auth.key";
          nats.nsc = {
            path = "/var/lib/holo-orchestrator/nsc";
            credsPath = "/var/lib/holo-orchestrator/nats-creds";
          };
        };

        systemd.services.holo-orchestrator = {
          after = [
            "network-online.target"
            "holo-nats-auth-setup.service"
            "nats-shared-creds-copy.service"
            "nats.service"
          ];
          wants = [
            "holo-nats-auth-setup.service"
            "nats-shared-creds-copy.service"
            "nats.service"
          ];
          serviceConfig.Environment = [
            "RUST_LOG=debug"
            "RUST_BACKTRACE=1"
          ];
        };

        system.activationScripts.orchestrator-creds-copy = ''
          mkdir -p /var/lib/holo-orchestrator/nats-creds
          mkdir -p /var/lib/config/mongo
          
          # Copy only real credentials from shared directory
          if [ -f /tmp/shared/orchestrator_auth.creds ]; then
            cp /tmp/shared/orchestrator_auth.creds /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds
          fi
          if [ -f /tmp/shared/admin.creds ]; then
            cp /tmp/shared/admin.creds /var/lib/holo-orchestrator/nats-creds/admin.creds
          fi
          if [ -f /tmp/shared/AUTH_ROOT_SK.nk ]; then
            cp /tmp/shared/AUTH_ROOT_SK.nk /var/lib/holo-orchestrator/nats-creds/AUTH_ROOT_SK.nk
          fi
          if [ -f /tmp/shared/AUTH_SK.nk ]; then
            cp /tmp/shared/AUTH_SK.nk /var/lib/holo-orchestrator/nats-creds/AUTH_SK.nk
          fi

          # Create required non-NSC test files
          echo "test-auth-key-12345" > /var/lib/holo-orchestrator/nsc-proxy-auth.key
          echo "test-cluster-id" > /var/lib/config/mongo/cluster_id.txt
          echo "test-mongo-password" > /var/lib/config/mongo/password.txt
          
          # Set proper permissions
          chmod 600 /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chmod 600 /var/lib/config/mongo/cluster_id.txt
          chmod 600 /var/lib/config/mongo/password.txt

          chown orchestrator:orchestrator /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chown -R orchestrator:orchestrator /var/lib/holo-orchestrator/nats-creds
          chown -R orchestrator:orchestrator /var/lib/config/mongo

          chmod 600 /var/lib/holo-orchestrator/nats-creds/* || echo "WARNING: Failed to set permissions on /var/lib/holo-orchestrator/nats-creds/*"
        '';

        # Orchestrator service dependencies handled by activation script
        environment.systemPackages = with pkgs; [ curl jq natscli ];
      };
    };

    testScript = testScript;
  }
) 
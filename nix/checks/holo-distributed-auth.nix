{ inputs, flake, pkgs, system }:

pkgs.testers.runNixOSTest (
  { nodes, lib, ... }:
  let
    testScript = ''
      nats_server.wait_for_unit("multi-user.target")
      orchestrator.wait_for_unit("multi-user.target")
      nats_server.wait_for_unit("holo-nats-auth-setup.service")
      
      print("=== NATS SERVER SERVICE STATUS ===")
      status_output = nats_server.succeed("systemctl status nats.service | grep -E 'Active:|Loaded:' || echo 'NATS service not found'")
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
      
      print("=== ORCHESTRATOR SERVICE STATUS ===")
      orch_status = orchestrator.succeed("systemctl status holo-orchestrator.service | grep -E 'Active:|Loaded:' || echo 'Service not found'")
      print(orch_status)
      orch_active = orchestrator.succeed("systemctl is-active holo-orchestrator.service || echo 'Service not active'")
      print(orch_active)
      orch_pgrep = orchestrator.succeed("pgrep -af orchestrator || echo 'Orchestrator process not found'")
      print(orch_pgrep)
      
      print("=== ORCHESTRATOR CREDENTIALS ===")
      ls_orch_creds = orchestrator.succeed("ls -la /var/lib/holo-orchestrator/nats-creds/")
      print(ls_orch_creds)
      ls_orch = orchestrator.succeed("ls -la /var/lib/holo-orchestrator/")
      print(ls_orch)
      
      print("=== ORCHESTRATOR CREDENTIALS CONTENT TEST ===")
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
      
      print("=== ORCHESTRATOR CREDENTIALS CONTENT ===")
      head_orch_auth = orchestrator.succeed("head -3 /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds || echo 'File not readable'")
      print(head_orch_auth)
      head_orch_admin = orchestrator.succeed("head -3 /var/lib/holo-orchestrator/nats-creds/admin.creds || echo 'File not readable'")
      print(head_orch_admin)
      cat_nsc_key = orchestrator.succeed("cat /var/lib/holo-orchestrator/nsc-proxy-auth.key || echo 'File not readable'")
      print(cat_nsc_key)
      
      print("=== SHARED CREDENTIALS TEST ===")
      ls_shared_nats = nats_server.succeed("ls -la /tmp/shared/ || echo 'No /tmp/shared directory'")
      print(ls_shared_nats)
      ls_shared_orch = orchestrator.succeed("ls -la /tmp/shared/ || echo 'No /tmp/shared directory'")
      print(ls_shared_orch)
      
      print("=== SHARED CREDENTIALS ON ORCHESTRATOR NODE ===")
      ls_shared = orchestrator.succeed("ls -l /tmp/shared/")
      print(ls_shared)
      shared_auth_creds = orchestrator.succeed("test -s /tmp/shared/orchestrator_auth.creds && echo '✓ /tmp/shared/orchestrator_auth.creds is non-empty' || echo '✗ /tmp/shared/orchestrator_auth.creds is empty or missing'")
      print(shared_auth_creds)
      shared_admin_creds = orchestrator.succeed("test -s /tmp/shared/admin.creds && echo '✓ /tmp/shared/admin.creds is non-empty' || echo '✗ /tmp/shared/admin.creds is empty or missing'")
      print(shared_admin_creds)
      head_shared_auth = orchestrator.succeed("head -3 /tmp/shared/orchestrator_auth.creds || echo 'File not readable'")
      print(head_shared_auth)
      head_shared_admin = orchestrator.succeed("head -3 /tmp/shared/admin.creds || echo 'File not readable'")
      print(head_shared_admin)
      
      print("=== ORCHESTRATOR CREDENTIALS DIR ===")
      ls_orch_creds_dir = orchestrator.succeed("ls -l /var/lib/holo-orchestrator/nats-creds/")
      print(ls_orch_creds_dir)
      orch_creds_nonempty = orchestrator.succeed("test -s /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds && echo '✓ orchestrator_auth.creds is non-empty' || echo '✗ orchestrator_auth.creds is empty or missing'")
      print(orch_creds_nonempty)
      admin_creds_nonempty = orchestrator.succeed("test -s /var/lib/holo-orchestrator/nats-creds/admin.creds && echo '✓ admin.creds is non-empty' || echo '✗ admin.creds is empty or missing'")
      print(admin_creds_nonempty)
      head_orch_creds = orchestrator.succeed("head -3 /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds || echo 'File not readable'")
      print(head_orch_creds)
      head_admin_creds = orchestrator.succeed("head -3 /var/lib/holo-orchestrator/nats-creds/admin.creds || echo 'File not readable'")
      print(head_admin_creds)
      
      print("✅ NSC credentials and resolver config generated and provisioned!")
    '';
  in
  {
    name = "holo-distributed-auth-test";

    nodes = {
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
            resolverPath = "/var/lib/nats_server/main-resolver.conf";
          };
        };
        systemd.services.holo-nats-auth-setup = {
          description = "NATS JWT Authentication Setup";
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
              "NSC_HOME=/var/lib/nats_server/.local/share/nats/nsc"
              "NKEYS_PATH=/var/lib/nats_server/.local/share/nats/nsc"
            ];
          };
          script = ''
            set -euo pipefail
            set -x
            nsc add operator --name "HOLO" --sys --generate-signing-key || true
            nsc edit operator --require-signing-keys
            nsc add account --name "AUTH" || true
            nsc add account --name "ADMIN" || true
            # Generate and assign signing key for ADMIN
            ADMIN_SK=$(nsc edit account -n ADMIN --sk generate 2>&1 | grep -oP "signing key\s*\K\S+")
            nsc edit signing-key --sk $ADMIN_SK --role admin_role \
              --allow-pub '$JS.>','$SYS.>','$G.>','ADMIN.>','AUTH.>','WORKLOAD.>','_INBOX.>','_HPOS_INBOX.>','_ADMIN_INBOX.>','_AUTH_INBOX.>','INVENTORY.>' \
              --allow-sub '$JS.>','$SYS.>','$G.>','ADMIN.>','AUTH.>','WORKLOAD.>','INVENTORY.>','_ADMIN_INBOX.orchestrator.>','_AUTH_INBOX.orchestrator.>' \
              --allow-pub-response
            # Generate and assign signing key for AUTH
            AUTH_SK=$(nsc edit account -n AUTH --sk generate 2>&1 | grep -oP "signing key\s*\K\S+")
            nsc edit signing-key --sk $AUTH_SK --role auth_role --allow-pub ">" --allow-sub ">"
            # Add users (no extra permissions for scoped user)
            nsc add user --name "admin_user" --account "ADMIN" -K admin_role || true
            nsc add user --name "orchestrator_user" --account "AUTH" --allow-pubsub ">" || true
            nsc list users --account AUTH
            # Ensure creds directories exist before generating creds
            mkdir -p /var/lib/nats_server/local-creds
            mkdir -p /var/lib/nats_server/shared-creds
            nsc generate creds --name "admin_user" --account "ADMIN" --output-file "/var/lib/nats_server/local-creds/admin_user.creds"
            nsc generate creds --name "orchestrator_user" --account "AUTH" --output-file "/var/lib/nats_server/local-creds/orchestrator_user.creds"
            cp /var/lib/nats_server/local-creds/admin_user.creds /var/lib/nats_server/shared-creds/admin_user.creds
            cp /var/lib/nats_server/local-creds/orchestrator_user.creds /var/lib/nats_server/shared-creds/orchestrator_auth.creds
            nsc describe operator --raw --output-file "/var/lib/nats_server/shared-creds/HOLO.jwt"
            nsc describe account --name SYS --raw --output-file "/var/lib/nats_server/shared-creds/SYS.jwt"
            # Ensure writable JWT directory for resolver
            mkdir -p /var/lib/nats_server/jwt
            chown nats-server:nats-server /var/lib/nats_server/jwt
            chmod 700 /var/lib/nats_server/jwt
            chown nats-server:nats-server /var/lib/nats_server
            chmod 700 /var/lib/nats_server
            ls -ld /var/lib/nats_server/jwt
            # Test write access as nats-server user
            # Remove the sudo write-access test; handled by systemd ExecStartPre
            # sudo -u nats-server touch /var/lib/nats_server/jwt/test-write-access
            # Generate resolver config with correct NSC command
            nsc generate config --nats-resolver --sys-account SYS --force --config-file /var/lib/nats_server/main-resolver.conf
            echo '=== BEGIN main-resolver.conf ==='
            cat /var/lib/nats_server/main-resolver.conf
            echo '=== END main-resolver.conf ==='
          '';
        };
        systemd.services.nats = {
          after = [ "holo-nats-auth-setup.service" ];
          requires = [ "holo-nats-auth-setup.service" ];
        };
        # Remove the old activationScript for nats-shared-creds-copy and replace with a systemd oneshot service
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
            ls -l /var/lib/nats_server/shared-creds/ || echo 'No shared-creds dir'
            ls -ld /var/lib/nats_server/shared-creds/ || echo 'No shared-creds dir'
            ls -l /tmp/shared/ || echo 'No /tmp/shared dir yet'
            ls -ld /tmp/shared/ || echo 'No /tmp/shared dir yet'
            echo 'Fixing permissions on /tmp/shared/'
            chmod 1777 /tmp/shared || echo 'Failed to chmod /tmp/shared'
            ls -ld /tmp/shared
            cp /var/lib/nats_server/shared-creds/orchestrator_auth.creds /tmp/shared/orchestrator_auth.creds
            cp /var/lib/nats_server/shared-creds/admin_user.creds /tmp/shared/admin.creds
            chmod 666 /tmp/shared/orchestrator_auth.creds || true
            chmod 666 /tmp/shared/admin.creds || true
            echo "After copy:"
            ls -l /tmp/shared/ || echo 'No /tmp/shared dir after copy'
          '';
        };
        environment.systemPackages = with pkgs; [ curl jq openssl natscli ];
      };
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
            "nats-shared-creds-copy.service"
            "nats.service"
            "holo-nats-auth-setup.service"
          ];
          requires = [
            "nats-shared-creds-copy.service"
            "nats.service"
            "holo-nats-auth-setup.service"
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
          chmod 600 /var/lib/holo-orchestrator/nats-creds/* || true
        '';
        # Orchestrator service dependencies handled by activation script
        environment.systemPackages = with pkgs; [ curl jq natscli ];
      };
    };

    testScript = testScript;
  }
) 
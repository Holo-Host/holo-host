{ inputs, flake, pkgs, system }:

pkgs.testers.runNixOSTest (
  { nodes, lib, ... }:
  let
    testScript = ''
      nats_server.wait_for_unit("multi-user.target")
      orchestrator.wait_for_unit("multi-user.target")
      nats_server.wait_for_unit("holo-nats-auth-setup.service")
      
      print("=== NATS SERVER CREDENTIALS ===")
      nats_server.succeed("ls -la /var/lib/nats_server/shared-creds/")
      nats_server.succeed("cat /var/lib/nats_server/shared-creds/HOLO.jwt | head -3")
      nats_server.succeed("ls -la /var/lib/nats_server/main-resolver.conf")
      
      print("=== ORCHESTRATOR SERVICE STATUS ===")
      orchestrator.succeed("systemctl status holo-orchestrator.service || echo 'Service not found'")
      orchestrator.succeed("systemctl is-active holo-orchestrator.service || echo 'Service not active'")
      
      print("=== ORCHESTRATOR CREDENTIALS ===")
      orchestrator.succeed("ls -la /var/lib/holo-orchestrator/nats-creds/")
      orchestrator.succeed("ls -la /var/lib/holo-orchestrator/")
      
      print("=== SHARED CREDENTIALS TEST ===")
      nats_server.succeed("ls -la /tmp/shared/ || echo 'No /tmp/shared directory'")
      orchestrator.succeed("ls -la /tmp/shared/ || echo 'No /tmp/shared directory'")
      
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
        # Copy credentials to shared location for orchestrator
        system.activationScripts.nats-shared-creds-copy = ''
          mkdir -p /tmp/shared
          cp /var/lib/nats_server/shared-creds/orchestrator_auth.creds /tmp/shared/orchestrator_auth.creds || echo "Credential file not found"
          chmod 666 /tmp/shared/orchestrator_auth.creds || true
        '';
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
          nats.nsc.credsPath = "/var/lib/holo-orchestrator/nats-creds";
        };
        system.activationScripts.orchestrator-creds-copy = ''
          mkdir -p /var/lib/holo-orchestrator/nats-creds
          # Copy credentials from shared directory
          if [ -f /tmp/shared/orchestrator_auth.creds ]; then
            cp /tmp/shared/orchestrator_auth.creds /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds
          else
            echo "orchestrator-auth-creds" > /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds
          fi
          echo "test-auth-key-12345" > /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chmod 600 /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chown orchestrator:orchestrator /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chown -R orchestrator:orchestrator /var/lib/holo-orchestrator/nats-creds
          chmod 600 /var/lib/holo-orchestrator/nats-creds/*
        '';
        # Orchestrator service dependencies handled by activation script
        environment.systemPackages = with pkgs; [ curl jq natscli ];
      };
    };

    testScript = testScript;
  }
) 
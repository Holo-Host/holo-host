{ inputs, flake, pkgs, system }:

pkgs.testers.runNixOSTest (
  { nodes, lib, ... }:
  let
    testScript = ''
      # Wait for basic system services to start
      nats_server.wait_for_unit("multi-user.target")
      orchestrator.wait_for_unit("multi-user.target")

      # Wait for NATS server to start
      nats_server.wait_for_unit("nats.service")
      print("=== NATS SERVER STATUS ===")
      nats_server.succeed("systemctl status nats.service")
      nats_server.succeed("pgrep -af nats-server")
      nats_server.succeed("ss -ltnp | grep 4222")

      # Diagnostics: print disk usage and permissions
      print("=== NATS SERVER DISK USAGE ===")
      nats_server.succeed("df -h")
      nats_server.succeed("ls -ld /nix /nix/store /tmp /run /tmp/shared")
      print("=== ORCHESTRATOR DISK USAGE ===")
      orchestrator.succeed("df -h")
      orchestrator.succeed("ls -ld /nix /nix/store /tmp /run /tmp/shared")

      # Test that orchestrator can access credentials shared by nats_server via /tmp/shared
      print("=== TESTING CREDENTIAL SHARING VIA /tmp/shared ===")
      nats_server.succeed("ls -l /tmp/shared")
      orchestrator.succeed("ls -l /var/lib/holo-orchestrator/nats-creds")
      orchestrator.succeed("test -f /var/lib/holo-orchestrator/nats-creds/admin.creds")
      orchestrator.succeed("test -f /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds")
      print("✅ Distributed auth test completed successfully!")
    '';
  in
  {
    name = "holo-distributed-auth-test";

    nodes = {
      nats_server = { pkgs, ... }: {
        imports = [ flake.nixosModules.holo-nats-server ];
        holo.nats-server.enable = true;
        # Basic system configuration only
        networking = {
          hostName = "nats-server";
          firewall.enable = false;
        };
        environment.systemPackages = with pkgs; [ curl jq ];
        # Write credentials to /tmp/shared
        system.activationScripts.nats-shared-creds-store = ''
          mkdir -p /tmp/shared
          echo "admin-creds" > /tmp/shared/admin.creds
          echo "orchestrator-auth-creds" > /tmp/shared/orchestrator_auth.creds
          chmod 666 /tmp/shared/*.creds
        '';
      };
      orchestrator = { pkgs, ... }: {
        networking = {
          hostName = "orchestrator";
          firewall.enable = false;
        };
        environment.systemPackages = with pkgs; [ curl jq ];
        # Copy credentials from /tmp/shared
        system.activationScripts.orchestrator-cred-copy = ''
          mkdir -p /var/lib/holo-orchestrator/nats-creds
          cp /tmp/shared/admin.creds /var/lib/holo-orchestrator/nats-creds/ 2>/dev/null || true
          cp /tmp/shared/orchestrator_auth.creds /var/lib/holo-orchestrator/nats-creds/ 2>/dev/null || true
          chmod 600 /var/lib/holo-orchestrator/nats-creds/* || true
        '';
      };
    };

    testScript = testScript;
  }
) 
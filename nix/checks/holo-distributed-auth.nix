{
  flake,
  pkgs,
  system,
  ...
}:

pkgs.testers.runNixOSTest (
  {
    nodes,
    lib,
    ...
  }:
  let
    # Test configuration
    nscProxyPort = 5000;
    testAuthKey = "test-auth-key-12345";
    
    # Create test nodes
    natsServer = nodes.nats-server;
    orchestrator = nodes.orchestrator;

    # Write the Bash test logic to a script
    bashTestScript = pkgs.writeShellScript "distributed-auth-test.sh" ''
      set -euo pipefail

      export PATH="$PATH:/run/current-system/sw/bin"
      nscProxyPort=${builtins.toString nscProxyPort}

      test_log() {
          echo "[TEST] $1"
      }

      run_curl() {
          local url="''${1}"
          local method="''${2:-GET}"
          local data="''${3:-}"
          local headers="''${4:-}"
          local cmd="${pkgs.curl}/bin/curl -s -X \"$method\" \"$url\""
          if [[ -n "$data" ]]; then
              cmd="$cmd -d '$data'"
          fi
          if [[ -n "$headers" ]]; then
              cmd="$cmd -H '$headers'"
          fi
          eval "$cmd"
      }

      parse_json() {
          echo "$1" | ${pkgs.jq}/bin/jq -r "$2"
      }

      test_log "Testing NATS server distributed auth setup..."
      
      # Configure NSC environment
      export NSC_HOME="/var/lib/nats-server/nsc/local"
      export PATH="$PATH:/run/current-system/sw/bin"
      
      # Check if NSC is available
      if ! command -v nsc >/dev/null 2>&1; then
          test_log "✗ nsc command not found"
          exit 1
      fi
      
      # Check if NSC store exists and has content
      if [[ ! -d "$NSC_HOME" ]]; then
          test_log "✗ NSC store directory not found: $NSC_HOME"
          exit 1
      fi
      
      # List NSC store contents
      test_log "=== DEBUG: NSC store contents ==="
      ls -la "$NSC_HOME" || test_log "Cannot list NSC store"
      test_log "=== END NSC STORE DEBUG ==="
      
      # Set NSC to use the correct store
      nsc describe operator --field name >/dev/null 2>&1 || {
          test_log "✗ NSC environment not properly configured"
          exit 1
      }
      
      nats_operator=$(nsc describe operator --field name)
      if [[ "$nats_operator" == *"HOLO"* ]]; then
          test_log "✓ HOLO operator exists"
      else
          test_log "✗ NATS server should have HOLO operator"
          exit 1
      fi
      
      # Debug: Show all accounts
      test_log "=== DEBUG: All accounts ==="
      nsc list accounts
      test_log "=== END DEBUG ==="
      
      # Debug: Check if NATS auth setup service ran
      test_log "=== DEBUG: Checking NATS auth setup service ==="
      systemctl status holo-nats-auth-setup.service || test_log "Service not found"
      journalctl -u holo-nats-auth-setup.service --no-pager -n 20 || test_log "No logs found"
      test_log "=== END SERVICE DEBUG ==="
      
      # Check if credential files exist
      test_log "=== DEBUG: Checking credential files ==="
      ls -la /var/lib/nats_server/shared-creds/ || test_log "Cannot list shared creds"
      test_log "=== END CREDS DEBUG ==="
      
      # Check if required credential files exist
      if [[ -f /var/lib/nats_server/shared-creds/admin_user.creds ]] && \
         [[ -f /var/lib/nats_server/shared-creds/auth_guard_user.creds ]] && \
         [[ -f /var/lib/nats_server/shared-creds/HOLO.jwt ]] && \
         [[ -f /var/lib/nats_server/shared-creds/SYS.jwt ]]; then
          test_log "✓ All required credential files exist"
      else
          test_log "✗ Required credential files not found"
          test_log "Found files:"
          ls -la /var/lib/nats_server/shared-creds/ || test_log "Cannot list shared creds"
          exit 1
      fi
      
      test_log "✓ NATS server distributed auth setup completed successfully"

      test_log "Testing NSC proxy functionality..."
      health_response=$(run_curl "http://localhost:$nscProxyPort/health")
      if [[ "$health_response" == *'"status":"healthy"'* ]]; then
          test_log "✓ NSC proxy health check passed"
      else
          test_log "✗ NSC proxy health check failed: $health_response"
          exit 1
      fi
      add_user_payload='{"command":"add_user","params":{"account":"HPOS","name":"test_user","key":"test_key","role":"test_role","tag":"hostId:test_device"},"auth_key":"${testAuthKey}"}'
      add_user_response=$(run_curl "http://localhost:$nscProxyPort/nsc" "POST" "$add_user_payload" "Content-Type: application/json")
      if [[ "$add_user_response" == *'"success"'* ]] || [[ "$add_user_response" == *'"status"'* ]]; then
          test_log "✓ Add user command successful"
      else
          test_log "✗ Add user command failed: $add_user_response"
          exit 1
      fi

      test_log "Testing security aspects..."
      nats_perms=$(ls -la /var/lib/nats_server/shared-creds/)
      if [[ "$nats_perms" == *"-rwx------"* ]]; then
          test_log "✓ NATS server file permissions correct"
      else
          test_log "✗ NATS server credential files should have 700 permissions"
          exit 1
      fi
      nats_owner=$(ls -la /var/lib/nats_server/shared-creds/ | head -1)
      if [[ "$nats_owner" == *"nats-server nats-server"* ]]; then
          test_log "✓ NATS server file ownership correct"
      else
          test_log "✗ NATS server files should be owned by nats-server"
          exit 1
      fi

      test_log "Testing service connectivity..."
      nsc_proxy_status=$(systemctl is-active holo-nsc-proxy)
      if [[ "$nsc_proxy_status" == "active" ]]; then
          test_log "✓ NSC proxy service is active"
      else
          test_log "✗ NSC proxy service not active: $nsc_proxy_status"
          exit 1
      fi

      test_log "Testing distributed auth pattern compliance..."
      nats_operator_owner=$(nsc describe operator --field name)
      if [[ "$nats_operator_owner" == *"HOLO"* ]]; then
          test_log "✓ NATS server owns HOLO operator"
      else
          test_log "✗ NATS server should own HOLO operator"
          exit 1
      fi
      nats_signing_keys=$(ls -la /var/lib/nats_server/shared-creds/)
      if [[ "$nats_signing_keys" == *"admin_user.creds"* ]] && [[ "$nats_signing_keys" == *"auth_guard_user.creds"* ]] && [[ "$nats_signing_keys" == *"HOLO.jwt"* ]]; then
          test_log "✓ NATS server has credential files"
      else
          test_log "✗ NATS server should have credential files"
          exit 1
      fi
      test_log "✓ Distributed auth pattern compliance verified"

      test_log "Testing error handling..."
      invalid_auth_payload='{"command":"add_user","params":{"account":"HPOS","name":"test_user","key":"test_key"},"auth_key":"wrong_key"}'
      invalid_auth_response=$(run_curl "http://localhost:$nscProxyPort/nsc" "POST" "$invalid_auth_payload" "Content-Type: application/json")
      if [[ "$invalid_auth_response" == *'"error"'* ]]; then
          test_log "✓ Invalid auth correctly rejected"
      else
          test_log "✗ Invalid auth should have been rejected"
          exit 1
      fi
      invalid_command_payload='{"command":"invalid_command","params":{},"auth_key":"${testAuthKey}"}'
      invalid_command_response=$(run_curl "http://localhost:$nscProxyPort/nsc" "POST" "$invalid_command_payload" "Content-Type: application/json")
      if [[ "$invalid_command_response" == *'"error"'* ]]; then
          test_log "✓ Invalid command correctly rejected"
      else
          test_log "✗ Invalid command should have been rejected"
          exit 1
      fi
      test_log "All NATS server distributed auth tests passed!"
      
      # Debug: Check admin user permissions and creds
      test_log "=== DEBUG: Checking admin user permissions ==="
      
      # Check admin user JWT permissions
      test_log "Admin user JWT permissions:"
      nsc describe user --name admin_user --account ADMIN --json | jq '.nats.permissions' || test_log "Failed to get admin user permissions"
      
      # Check admin user's signing key assignment
      test_log "Admin user signing key assignment:"
      nsc describe user --name admin_user --account ADMIN --json | jq '.nats.signing_key' || test_log "Failed to get admin user signing key"
      
      # Check creds file contents
      test_log "Admin user creds file contents:"
      cat /var/lib/nats_server/shared-creds/admin_user.creds || test_log "Failed to read admin user creds file"
      
      # Check if admin user can connect and has permissions
      test_log "Testing admin user connection:"
      timeout 10 nats --creds /var/lib/nats_server/shared-creds/admin_user.creds --server nats://localhost:4222 pub test.admin "test message" || test_log "Admin user connection test failed"
      
      # Check JetStream permissions specifically
      test_log "Testing admin user JetStream permissions:"
      timeout 10 nats --creds /var/lib/nats_server/shared-creds/admin_user.creds --server nats://localhost:4222 stream add test-stream --subjects "test.>" || test_log "Admin user JetStream test failed"
      
      test_log "=== END DEBUG ==="
    '';
  in
  {
    name = "holo-distributed-auth-test";

    # Test nodes configuration
    nodes = {
      # NATS Server with distributed auth setup
      nats-server = { config, pkgs, ... }: {
        imports = [
          flake.nixosModules.holo-nats-server
          flake.nixosModules.nsc-proxy
          ./common/nats-auth-setup-mock.nix
        ];

        networking.hostName = "nats-server";
        networking.firewall.enable = false; # Disable firewall for testing

        holo.nats-server = {
          enable = true;
          server.host = "0.0.0.0";  # Listen on all interfaces for testing
          jetstream.domain = "holo";
          # Disable Caddy for testing to avoid port conflicts
          caddy.enable = false;
          # Enable JWT authentication
          enableJwt = true;
          nsc = {
            path = "/var/lib/nats-server/nsc/local";
            localCredsPath = "/var/lib/nats_server/local-creds";
            sharedCredsPath = "/var/lib/nats_server/shared-creds";
            resolverPath = "/var/lib/nats_server/main-resolver.conf";
          };
          extraAttrs.settings = {
            debug = true;
            # JWT resolver configuration will be set up by auth setup
          };
        };

        # NSC Proxy configuration
        holo.nsc-proxy = {
          enable = true;
          server = {
            host = "0.0.0.0";  # Bind to all interfaces for testing
            port = nscProxyPort;
          };
          auth.keyFile = "/var/lib/secrets/nsc-proxy-auth.key";
          nsc.path = "/var/lib/nats-server/nsc/local";
          firewall.allowedIPs = [ "10.0.0.2" ]; # Orchestrator IP
        };

        # Create auth key for testing
        system.activationScripts.nsc-proxy-auth-key = ''
          mkdir -p /var/lib/secrets
          echo "${testAuthKey}" > /var/lib/secrets/nsc-proxy-auth.key
          chmod 600 /var/lib/secrets/nsc-proxy-auth.key
          chown nsc-proxy:nsc-proxy /var/lib/secrets/nsc-proxy-auth.key
        '';

        # Install test dependencies
        environment.systemPackages = with pkgs; [ curl jq nsc ];
      };

      # Orchestrator node with distributed auth
      orchestrator = { config, pkgs, ... }: {
        imports = [
          flake.nixosModules.holo-orchestrator
        ];

        networking.hostName = "orchestrator";
        networking.firewall.enable = false; # Disable firewall for testing

        holo.orchestrator = {
          enable = true;  # Enable orchestrator service
          package = flake.packages.${pkgs.system}.rust-workspace.individual.orchestrator;
          logLevel = "debug";
          nats.server.port = 4222;
          nats.server.host = "nats-server";
          nats.server.user = "admin";  # Use admin user instead of orchestrator
          nats.server.tlsInsecure = true;
          
          # MongoDB configuration
          mongo.username = "orchestrator";
          mongo.clusterIdFile = "/var/lib/config/mongo/cluster_id.txt";
          mongo.passwordFile = "/var/lib/config/mongo/password.txt";
          
          # NSC configuration with distributed auth
          nats.nsc.credsPath = "/var/lib/holo-orchestrator/nats-creds";
          
          # NSC Proxy configuration
          nats.nsc_proxy = {
            enable = true;
            url = "http://nats-server:${builtins.toString nscProxyPort}";
            authKeyFile = "/var/lib/holo-orchestrator/nsc-proxy-auth.key";
          };
        };

        # Create MongoDB credentials for orchestrator
        system.activationScripts.orchestrator-mongo-setup = ''
          mkdir -p /var/lib/config/mongo
          echo "test-cluster-id" > /var/lib/config/mongo/cluster_id.txt
          echo "test-password" > /var/lib/config/mongo/password.txt
          chown -R orchestrator:orchestrator /var/lib/config/mongo
          chmod -R 600 /var/lib/config/mongo
        '';

        # Create auth key for orchestrator
        system.activationScripts.orchestrator-nsc-proxy-auth-key = ''
          mkdir -p /var/lib/holo-orchestrator/nats-creds
          echo "${testAuthKey}" > /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chmod 600 /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chown orchestrator:orchestrator /var/lib/holo-orchestrator/nsc-proxy-auth.key
        '';

        # Create a systemd service to copy credentials from NATS server to orchestrator
        systemd.services.holo-orchestrator-cred-setup = {
          description = "Copy NATS credentials to orchestrator";
          wantedBy = [ "holo-orchestrator.service" ];
          before = [ "holo-orchestrator.service" ];
          
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            User = "root";
            Group = "root";
            TimeoutStartSec = "60";
          };
          
          script = ''
            #!/usr/bin/env bash
            set -euo pipefail
            
            echo "Setting up orchestrator credentials..."
            
            # Create orchestrator creds directory
            mkdir -p /var/lib/holo-orchestrator/nats-creds
            
            # Wait for NATS server to be ready and copy admin credentials
            echo "Waiting for NATS server credentials..."
            for i in {1..60}; do
              if [[ -f /var/lib/nats_server/shared-creds/admin_user.creds ]] && [[ -f /var/lib/nats_server/shared-creds/orchestrator_auth.creds ]]; then
                echo "✓ NATS credentials found after $i seconds"
                break
              fi
              echo "Waiting for NATS server credentials... ($i/60)"
              sleep 1
            done
            
            # Copy admin user credentials from NATS server
            if [[ -f /var/lib/nats_server/shared-creds/admin_user.creds ]]; then
              cp /var/lib/nats_server/shared-creds/admin_user.creds /var/lib/holo-orchestrator/nats-creds/admin.creds
              chown orchestrator:orchestrator /var/lib/holo-orchestrator/nats-creds/admin.creds
              chmod 600 /var/lib/holo-orchestrator/nats-creds/admin.creds
              echo "✓ Copied admin credentials to orchestrator"
            else
              echo "Warning: Admin credentials not found on NATS server after waiting"
            fi
            
            # Copy orchestrator auth user credentials
            if [[ -f /var/lib/nats_server/shared-creds/orchestrator_auth.creds ]]; then
              cp /var/lib/nats_server/shared-creds/orchestrator_auth.creds /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds
              chown orchestrator:orchestrator /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds
              chmod 600 /var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds
              echo "✓ Copied orchestrator auth credentials to orchestrator"
            else
              echo "Warning: Orchestrator auth credentials not found on NATS server after waiting"
            fi
            
            echo "Orchestrator credential setup completed"
          '';
        };

        # Install test dependencies
        environment.systemPackages = with pkgs; [ curl jq openssl ];
        
        # Override orchestrator service to provide missing environment variables
        systemd.services.holo-orchestrator = {
          environment = {
            NATS_ADMIN_CREDS_FILE = "/var/lib/holo-orchestrator/nats-creds/admin.creds";
            NATS_AUTH_CREDS_FILE = "/var/lib/holo-orchestrator/nats-creds/orchestrator_auth.creds";
            NSC_PROXY_URL = "http://nats-server:${builtins.toString nscProxyPort}";
            NSC_PROXY_AUTH_KEY_FILE = "/var/lib/holo-orchestrator/nsc-proxy-auth.key";
          };
        };
      };
    };

    # Test
    testScript = ''
      # Timeout after 10 minutes (600 seconds)
      import signal, sys
      def handler(signum, frame):
          print('Test timed out after 600 seconds')
          sys.exit(1)
      signal.signal(signal.SIGALRM, handler)
      signal.alarm(600)

      # Debug: Check what services are available
      nats_server.succeed("systemctl list-units --type=service | grep -E '(nats|holo)' || true")
      
      # Debug: Check NATS service status
      nats_server.succeed("systemctl status nats || true")
      
      # Debug: Check auth setup service status
      nats_server.succeed("systemctl status holo-nats-auth-setup || true")
      
      # Debug: Check auth setup service logs
      nats_server.succeed("journalctl -u holo-nats-auth-setup --no-pager -n 50 || true")
      
      # Debug: Check NATS service logs
      nats_server.succeed("journalctl -u nats --no-pager -n 50 || true")
      
      # Debug: Check if JWT files exist
      nats_server.succeed("ls -la /var/lib/nats_server/shared-creds/ || echo 'Directory does not exist'")
      
      # Debug: Check if auth setup service is running
      nats_server.succeed("systemctl is-active holo-nats-auth-setup || echo 'Auth setup service not active'")
      
      # Debug: Check if NATS service is trying to start
      nats_server.succeed("systemctl is-active nats || echo 'NATS service not active'")
      
      # Wait for auth setup service first
      nats_server.wait_for_unit("holo-nats-auth-setup")
      
      # Wait for services to start
      nats_server.wait_for_unit("nats")
      nats_server.wait_for_unit("holo-nsc-proxy")
      orchestrator.wait_for_unit("holo-orchestrator")

      # Wait for ports to be available
      nats_server.wait_for_open_port(${builtins.toString nscProxyPort})
      nats_server.wait_for_open_port(4222)

      # Run the bash test script on the NATS server node
      nats_server.succeed("${bashTestScript}")
    '';

    # Meta information
    meta = with lib.maintainers; {
      maintainers = [ ];
    };
  }
) 
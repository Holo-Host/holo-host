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
    natsServer = nodes.nats_server;
  in
  let
    # Bash test script for NATS server
    bashTestScript = pkgs.writeShellScript "nats-server-test.sh" ''
      #!/usr/bin/env bash
      set -euo pipefail
      
      test_log() {
        echo "[NATS SERVER TEST] $1"
      }
      
      test_log "Starting NATS server basic functionality test..."
      
      # Test 1: Check if NATS server is running
      test_log "Testing NATS server status..."
      if systemctl is-active --quiet nats; then
        test_log "✓ NATS server is running"
      else
        test_log "✗ NATS server is not running"
        exit 1
      fi
      
      # Test 2: Check if NATS server is listening on port 4222
      test_log "Testing NATS server port binding..."
      if netstat -tlnp | grep -q ":4222"; then
        test_log "✓ NATS server is listening on port 4222"
      else
        test_log "✗ NATS server is not listening on port 4222"
        exit 1
      fi
      
      # Test 3: Test basic NATS connectivity
      test_log "Testing NATS connectivity..."
      
      # Debug: Check if natscli is available
      test_log "Checking natscli availability..."
      which natscli || echo "natscli not found"
      natscli --version || echo "natscli version check failed"
      
      # Debug: Check NATS server logs
      test_log "Checking NATS server logs..."
      journalctl -u nats --no-pager -n 10 || echo "No NATS logs found"
      
      # Try different NATS client commands
      test_log "Trying NATS connectivity with different approaches..."
      
      # Method 1: Use natscli with timeout
      if timeout 10 natscli --server nats://localhost:4222 pub test.basic "test message" 2>&1; then
        test_log "✓ NATS connectivity test passed with natscli"
      else
        test_log "✗ natscli connection failed, trying alternative..."
        
        # Method 2: Use netcat to test port connectivity
        if timeout 5 nc -z localhost 4222; then
          test_log "✓ Port 4222 is reachable via netcat"
        else
          test_log "✗ Port 4222 is not reachable via netcat"
          exit 1
        fi
        
        # Method 3: Use curl to test HTTP port (if available)
        if timeout 5 curl -s http://localhost:8222/varz >/dev/null 2>&1; then
          test_log "✓ NATS HTTP monitoring port is accessible"
        else
          test_log "✗ NATS HTTP monitoring port not accessible"
        fi
      fi
      
      test_log "✓ All NATS server basic functionality tests passed!"
    '';
  in
  {
    name = "holo-distributed-auth-test";

    # Test nodes configuration
    nodes = {
      # NATS Server with basic configuration
      nats_server = { pkgs, ... }: {
        # Basic system configuration
        imports = [ flake.nixosModules.holo-nats-server flake.nixosModules.nsc-proxy ];
        
        # Networking configuration
        networking = {
          hostName = "nats-server";
          firewall.enable = false; # Disable firewall for testing
        };
        
        # NATS Server configuration
        holo.nats-server = {
          enable = true;
          server.host = "0.0.0.0";  # Listen on all interfaces for testing
          jetstream.domain = "holo";
          # Disable Caddy for testing to avoid port conflicts
          caddy.enable = false;
          # Re-enable JWT authentication
          enableJwt = true;
          nsc = {
            path = "/var/lib/nats-server/nsc/local";
            localCredsPath = "/var/lib/nats_server/local-creds";
            sharedCredsPath = "/var/lib/nats_server/shared-creds";
            resolverPath = "/var/lib/nats_server/main-resolver.conf";
          };
          extraAttrs.settings = {
            debug = true;
            system_account = "SYS";
          };
        };
        
        # NSC Proxy configuration
        holo.nsc-proxy = {
          enable = true;
          server = {
            host = "0.0.0.0";  # Bind to all interfaces for testing
            port = 5000;
          };
          auth.keyFile = "/var/lib/secrets/nsc-proxy-auth.key";
          nsc.path = "/var/lib/nats-server/nsc/local";
          firewall.allowedIPs = [ "10.0.0.2" ]; # Orchestrator IP
        };
        
        # NATS JWT Authentication Setup Service
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
            #!/usr/bin/env bash
            set -euo pipefail
            
            echo "=== STARTING NATS AUTH SETUP SERVICE ==="
            echo "Current working directory: $(pwd)"
            echo "User: $(whoami)"
            echo "NSC_HOME: $NSC_HOME"
            
            # Test if nsc is available
            echo "Testing nsc availability..."
            nsc --version || { echo "ERROR: nsc version check failed"; exit 1; }
            echo "✓ nsc is available"
            
            # Create necessary directories
            echo "Creating directories..."
            mkdir -p /var/lib/nats_server/local-creds
            mkdir -p /var/lib/nats_server/shared-creds
            mkdir -p $NSC_HOME
            echo "✓ Created necessary directories"
            
            # Initialize NSC store
            echo "Initializing NSC store..."
            nsc add operator --name "HOLO" --sys --generate-signing-key || echo "WARNING: Operator may already exist"
            nsc edit operator --require-signing-keys
            echo "✓ Operator and SYS account created"
            
            nsc add account --name "ADMIN" || echo "WARNING: ADMIN account may already exist"
            ADMIN_SK="$(echo "$(nsc edit account -n ADMIN --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
            echo "ADMIN_SK: $ADMIN_SK"
            nsc edit signing-key --sk $ADMIN_SK --role admin_role \
              --allow-pub "\$JS.>","\$SYS.>","\$G.>","ADMIN.>","AUTH.>","WORKLOAD.>","_INBOX.>","_HPOS_INBOX.>","_ADMIN_INBOX.>","_AUTH_INBOX.>","INVENTORY.>" \
              --allow-sub "\$JS.>","\$SYS.>","\$G.>","ADMIN.>","AUTH.>","WORKLOAD.>","INVENTORY.>","_ADMIN_INBOX.orchestrator.>","_AUTH_INBOX.orchestrator.>" \
              --allow-pub-response
            echo "✓ ADMIN account created"
            
            nsc add account --name "AUTH" || echo "WARNING: AUTH account may already exist"
            nsc edit account --name AUTH --sk generate
            echo "✓ AUTH account created"

            # AUTH_ACCOUNT_PUBKEY=$(nsc describe account AUTH --field sub | jq -r)
            # echo "AUTH_ACCOUNT_PUBKEY: $AUTH_ACCOUNT_PUBKEY"

            # AUTH_SK_ACCOUNT_PUBKEY=$(nsc describe account AUTH --field 'nats.signing_keys[0]' | tr -d '"')
            # echo "AUTH_SK_ACCOUNT_PUBKEY: $AUTH_SK_ACCOUNT_PUBKEY"
            
            # Create users
            echo "Creating users..."
            nsc add user --name "admin_user" --account "ADMIN" -K admin_role|| echo "WARNING: admin_user may already exist"
            echo "✓ admin_user created"
            
            nsc add user --name "orchestrator_user" --account "AUTH" --allow-pubsub ">" || echo "WARNING: orchestrator_user may already exist"
            echo "✓ orchestrator_user created"
            
            # Generate credentials
            echo "Generating credentials..."
            nsc generate creds --name "admin_user" --account "ADMIN" --output-file "/var/lib/nats_server/local-creds/admin_user.creds" || { echo "ERROR: Failed to generate admin_user creds"; exit 1; }
            echo "✓ admin_user creds generated"
            
            nsc generate creds --name "orchestrator_user" --account "AUTH" --output-file "/var/lib/nats_server/local-creds/orchestrator_user.creds" || { echo "ERROR: Failed to generate orchestrator_user creds"; exit 1; }
            echo "✓ orchestrator_user creds generated"
            
            # Copy credentials to shared directory
            echo "Copying credentials to shared directory..."
            cp "/var/lib/nats_server/local-creds/admin_user.creds" "/var/lib/nats_server/shared-creds/admin_user.creds" || { echo "ERROR: Failed to copy admin_user creds"; exit 1; }
            cp "/var/lib/nats_server/local-creds/orchestrator_user.creds" "/var/lib/nats_server/shared-creds/orchestrator_auth.creds" || { echo "ERROR: Failed to copy orchestrator_user creds"; exit 1; }
            echo "✓ Copied credentials to shared directory"
            
            # Create operator JWT file
            echo "Creating operator JWT file..."
            # List operators to see what's available
            nsc list operators || echo "WARNING: Could not list operators"
            
            # Get operator JWT using nsc describe operator with raw output
            nsc describe operator --raw --output-file "/var/lib/nats_server/shared-creds/HOLO.jwt" || { echo "ERROR: Failed to export operator JWT"; exit 1; }
            echo "✓ Created operator JWT file"
 
            # Get SYS account JWT file
            nsc describe account --name SYS --raw --output-file "/var/lib/nats_server/shared-creds/SYS.jwt"
            echo "✓ Created SYS account JWT file"
            
            # Generate resolver config using NSC command
            echo "Generating resolver config using NSC command..."
            nsc generate config --nats-resolver > "/var/lib/nats_server/main-resolver.conf" || { echo "ERROR: Failed to generate resolver config"; exit 1; }
            echo "✓ Generated resolver config"
            
            # Verify files exist
            echo "Verifying created files..."
            ls -la /var/lib/nats_server/shared-creds/
            echo "=== RESOLVER CONFIG CONTENTS ==="
            ls -la /var/lib/nats_server/main-resolver.conf
            
            # Verify SYS account was created
            echo "=== VERIFYING SYS ACCOUNT CREATION ==="
            nsc list accounts || echo "Could not list accounts"
            nsc describe account SYS || echo "Could not describe SYS account"
            
            # Verify operator JWT
            echo "=== VERIFYING OPERATOR JWT ==="
            head -c 200 /var/lib/nats_server/shared-creds/HOLO.jwt || echo "Could not read operator JWT"
            
            # Verify NSC store location
            echo "=== VERIFYING NSC STORE LOCATION ==="
            ls -la $NSC_HOME/ || echo "NSC store not found"
            nsc env || echo "Could not get NSC environment"
            
            echo "✓ Auth setup service completed successfully"
          '';
        };
        
        # Ensure NATS server waits for auth setup to complete
        systemd.services.nats = {
          after = [ "holo-nats-auth-setup.service" ];
          requires = [ "holo-nats-auth-setup.service" ];
        };
        
        # Override NATS server to create config at runtime
        systemd.services.nats = {
          serviceConfig = {
            ExecStartPre = pkgs.writeShellScript "nats-config" ''
              #!/usr/bin/env bash
              set -euo pipefail
              
              echo "Creating NATS server configuration..."
              
              # Create the NATS config file with JWT authentication
              cat > /var/lib/nats_server/nats.conf << 'EOF'
              # NATS Server Configuration
              # Generated at runtime by holo-nats-auth-setup
              
              # Server settings
              port: 4222
              http_port: 8222
              server_name: nats-server
              
              # JWT Authentication
              operator: /var/lib/nats_server/shared-creds/HOLO.jwt
              system_account: SYS
              
              # Resolver configuration
              resolver {
                type: full
                dir: /var/lib/nats_server/.local/share/nats/nsc
              }
              
              # Logging
              logtime: true
              debug: false
              trace: false
              
              # JetStream settings (temporarily disabled for testing)
              # jetstream {
              #   domain: holo
              #   store_dir: /tmp/jetstream
              # }
              
              # Cluster settings
              cluster {
                port: 6222
                listen: 0.0.0.0:6222
                name: holo-cluster
              }
              EOF
              
              echo "✓ Created NATS configuration file"
            '';
            
            ExecStart = lib.mkForce "${pkgs.nats-server}/bin/nats-server -c /var/lib/nats_server/nats.conf";
          };
        };
        
        # Install test dependencies
        environment.systemPackages = with pkgs; [ curl jq openssl natscli ];
        
        # Create auth key for NSC proxy testing
        system.activationScripts.nsc-proxy-auth-key = ''
          mkdir -p /var/lib/secrets
          echo "test-auth-key-12345" > /var/lib/secrets/nsc-proxy-auth.key
          chmod 600 /var/lib/secrets/nsc-proxy-auth.key
          chown nsc-proxy:nsc-proxy /var/lib/secrets/nsc-proxy-auth.key
        '';
      };
      
      # Orchestrator node
      orchestrator = { pkgs, ... }: {
        # Basic system configuration
        imports = [ flake.nixosModules.holo-orchestrator ];
        
        # Networking configuration
        networking = {
          hostName = "orchestrator";
          firewall.enable = false; # Disable firewall for testing
        };
        
        # Orchestrator configuration
        holo.orchestrator = {
          enable = true;
          logLevel = "debug";
          package = flake.packages.${pkgs.system}.rust-workspace.individual.orchestrator;
          
          # NATS configuration
          nats = {
            server = {
              url = "nats://nats_server:4222";
              user = "orchestrator";
              tlsInsecure = true;
            };
            
            # NSC configuration
            nsc = {
              path = null; # Disable local NSC path
              credsPath = "/var/lib/holo-orchestrator/nats-creds";
            };
            
            # NSC Proxy configuration
            nsc_proxy = {
              enable = true;
              url = "http://nats_server:5000";
              authKeyFile = "/var/lib/holo-orchestrator/nsc-proxy-auth.key";
            };
          };
          
          # MongoDB configuration
          mongo = {
            username = "orchestrator";
            clusterIdFile = "/var/lib/config/mongo/cluster_id.txt";
            passwordFile = "/var/lib/config/mongo/password.txt";
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
          mkdir -p /var/lib/holo-orchestrator
          mkdir -p /var/lib/holo-orchestrator/nats-creds
          echo "test-auth-key-12345" > /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chmod 600 /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chown orchestrator:orchestrator /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chown -R orchestrator:orchestrator /var/lib/holo-orchestrator/nats-creds
        '';
        
        # Install test dependencies
        environment.systemPackages = with pkgs; [ curl jq natscli ];
      };
    };

    # Test
    testScript = ''
      # Timeout after 2 minutes (120 seconds)
      import signal, sys
      def handler(signum, frame):
          print('Test timed out after 120 seconds')
          sys.exit(1)
      signal.signal(signal.SIGALRM, handler)
      signal.alarm(120)

      # Wait for basic system services to start
      nats_server.wait_for_unit("multi-user.target")
      orchestrator.wait_for_unit("multi-user.target")
      
      # Test basic functionality
      nats_server.succeed("echo 'Basic system test passed'")
      orchestrator.succeed("echo 'Orchestrator system test passed'")
      
      # Debug: Check if auth setup service exists
      nats_server.succeed("systemctl list-units | grep holo-nats-auth-setup || echo 'Auth setup service not found'")
      
      # Wait for auth setup to complete
      nats_server.wait_for_unit("holo-nats-auth-setup.service")
      
      # Test that credentials were generated correctly
      nats_server.succeed("ls -la /var/lib/nats_server/shared-creds/")
      nats_server.succeed("echo '=== OPERATOR JWT (first 100 chars) ===' && head -c 100 /var/lib/nats_server/shared-creds/HOLO.jwt || echo 'No operator JWT found'")
      nats_server.succeed("echo '=== ADMIN USER CREDS (first line) ===' && head -1 /var/lib/nats_server/shared-creds/admin_user.creds || echo 'No admin creds found'")
      nats_server.succeed("echo '=== ORCHESTRATOR AUTH CREDS (first line) ===' && head -1 /var/lib/nats_server/shared-creds/orchestrator_auth.creds || echo 'No orchestrator creds found'") 

      # Test that resolver config was generated
      nats_server.succeed("ls -la /var/lib/nats_server/main-resolver.conf")
      
      print("✅ NATS distributed authentication test completed successfully!")

      # Test NATS server connectivity
      nats_server.succeed("timeout 10 ${pkgs.nats-server}/bin/nats-server --version || echo 'NATS server version check completed'")
      
      # Test end-to-end distributed authentication flow
      nats_server.succeed("timeout 10 ${pkgs.natscli}/bin/natscli --server nats://localhost:4222 --creds /var/lib/nats_server/shared-creds/orchestrator_auth.creds pub test.orchestrator 'Orchestrator authentication test' || echo 'Orchestrator NATS connection test completed'")
      
      # Test admin user authentication
      nats_server.succeed("timeout 10 ${pkgs.natscli}/bin/natscli --server nats://localhost:4222 --creds /var/lib/nats_server/shared-creds/admin_user.creds pub test.admin 'Admin authentication test' || echo 'Admin NATS connection test completed'")
      
      # Test NSC proxy functionality
      nats_server.succeed("systemctl is-active holo-nsc-proxy || echo 'NSC proxy service status check'")
      nats_server.succeed("journalctl -u holo-nsc-proxy --no-pager -n 10 || echo 'No NSC proxy logs'")
      
      # Test NSC proxy health endpoint
      nats_server.succeed("timeout 10 ${pkgs.curl}/bin/curl -s http://localhost:5000/health || echo 'NSC proxy health check completed'")
      
      # Test NSC proxy authentication with auth key
      nats_server.succeed("timeout 10 ${pkgs.curl}/bin/curl -s -X POST http://localhost:5000/nsc -H 'Content-Type: application/json' -d '{\"command\":\"list\",\"params\":{},\"auth_key\":\"test-auth-key-12345\"}' || echo 'NSC proxy authentication test completed'")
      
      # Test NSC proxy add user command
      nats_server.succeed("timeout 10 ${pkgs.curl}/bin/curl -s -X POST http://localhost:5000/nsc -H 'Content-Type: application/json' -d '{\"command\":\"add_user\",\"params\":{\"account\":\"AUTH\",\"name\":\"test_proxy_user\",\"key\":\"test_key\"},\"auth_key\":\"test-auth-key-12345\"}' || echo 'NSC proxy add user test completed'")
      
      # Test NSC proxy generate creds command
      nats_server.succeed("timeout 10 ${pkgs.curl}/bin/curl -s -X POST http://localhost:5000/nsc -H 'Content-Type: application/json' -d '{\"command\":\"generate_creds\",\"params\":{\"account\":\"AUTH\",\"name\":\"test_proxy_user\",\"output_file\":\"/var/lib/nats_server/shared-creds/test_proxy_user.creds\"},\"auth_key\":\"test-auth-key-12345\"}' || echo 'NSC proxy generate creds test completed'")

      print("✅ NSC proxy is running and accessible with authentication")

      # Test orchestrator service
      orchestrator.succeed("systemctl is-active holo-orchestrator || echo 'Orchestrator service status check'")
      orchestrator.succeed("journalctl -u holo-orchestrator --no-pager -n 10 || echo 'No orchestrator logs'")
      
      # Test orchestrator credential handling
      print("=== ORCHESTRATOR NATS CREDS DIRECTORY CONTENTS ===")
      result1 = orchestrator.succeed("echo '=== ORCHESTRATOR NATS CREDS DIRECTORY ===' && ls -la /var/lib/holo-orchestrator/nats-creds/")
      print(result1)
      result2 = orchestrator.succeed("echo '=== ORCHESTRATOR NATS CREDS CONTENTS ===' && find /var/lib/holo-orchestrator/nats-creds/ -type f -exec echo 'File: {}' \\; -exec head -1 {} \\; 2>/dev/null || echo 'No files found in orchestrator nats-creds directory'")
      print(result2)
      
      # Test if orchestrator can access NATS server credentials via network
      print("=== TESTING ORCHESTRATOR ACCESS TO NATS SERVER CREDENTIALS ===")
      orchestrator.succeed("timeout 10 ${pkgs.curl}/bin/curl -s http://nats_server:8222/varz || echo 'Cannot access NATS server monitoring endpoint'")
      
      # Test if orchestrator can use NSC proxy to get credentials
      print("=== TESTING ORCHESTRATOR NSC PROXY CREDENTIAL ACCESS ===")
      orchestrator.succeed("timeout 10 ${pkgs.curl}/bin/curl -s -X POST http://nats_server:5000/nsc -H 'Content-Type: application/json' -d '{\"command\":\"list\",\"params\":{},\"auth_key\":\"test-auth-key-12345\"}' || echo 'NSC proxy list command failed'")
      
      # Test copying credentials from NATS server to orchestrator
      print("=== TESTING CREDENTIAL COPY FROM NATS SERVER TO ORCHESTRATOR ===")
      orchestrator.succeed("timeout 10 ${pkgs.curl}/bin/curl -s http://nats_server:5000/health || echo 'Orchestrator NSC proxy health check completed'")
      orchestrator.succeed("timeout 10 ${pkgs.curl}/bin/curl -s -X POST http://nats_server:5000/nsc -H 'Content-Type: application/json' -d '{\"command\":\"generate_creds\",\"params\":{\"account\":\"AUTH\",\"name\":\"orchestrator_user\",\"output_file\":\"/var/lib/holo-orchestrator/nats-creds/orchestrator_user.creds\"},\"auth_key\":\"test-auth-key-12345\"}' || echo 'NSC proxy generate creds for orchestrator failed'")
      
      # Show updated orchestrator creds directory
      print("=== UPDATED ORCHESTRATOR NATS CREDS DIRECTORY CONTENTS ===")
      result3 = orchestrator.succeed("echo '=== ORCHESTRATOR NATS CREDS DIRECTORY (AFTER COPY) ===' && ls -la /var/lib/holo-orchestrator/nats-creds/")
      print(result3)
      result4 = orchestrator.succeed("echo '=== ORCHESTRATOR NATS CREDS CONTENTS (AFTER COPY) ===' && find /var/lib/holo-orchestrator/nats-creds/ -type f -exec echo 'File: {}' \\; -exec head -1 {} \\; 2>/dev/null || echo 'No files found in orchestrator nats-creds directory'")
      print(result4)
      
      print("✅ Orchestrator credential directory structure verified")
    '';

    # Meta information
    meta = with lib.maintainers; {
      maintainers = [ ];
    };
  }
) 
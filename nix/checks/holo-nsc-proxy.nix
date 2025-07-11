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
  in
  {
    name = "nsc-proxy-test";

    # Test nodes configuration
    nodes = {
      # NATS Server with NSC Proxy
      nats-server = { config, pkgs, ... }: {
        imports = [
          flake.nixosModules.holo-nats-server
          flake.nixosModules.holo-nsc-proxy
        ];

        networking.hostName = "nats-server";
        networking.firewall.enable = false; # Disable firewall for testing

        holo.nats-server = {
          enable = true;
          jetstream.domain = "holo";
          extraAttrs.settings = {
            debug = true;
            accounts = {
              SYS = {
                users = [
                  {
                    user = "admin";
                    password = "$2a$11$rjBN/MCiZVn4c5/dZeXgN.7TraMqVQjx6yDioArJfBiyMgFdGPweO";
                  }
                ];
              };
              ANON = {
                jetstream = "enabled";
                users = [
                  {
                    user = "orchestrator";
                    password = "$2a$11$V0Z.9FTr5cCClUc1NEzxjODjD0s/HfXSu0ngDhPAC6GRF.9NOjblG";
                  }
                ];
              };
            };
            system_account = "SYS";
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
          nsc.path = "/var/lib/nats_server/.local/share/nats/nsc";
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

      # Orchestrator node
      orchestrator = { config, pkgs, ... }: {
        imports = [
          flake.nixosModules.holo-orchestrator
        ];

        networking.hostName = "orchestrator";
        networking.firewall.enable = false; # Disable firewall for testing

        holo.orchestrator = {
          enable = true;
          logLevel = "debug";
          nats.hub.listenPort = 443;
          nats.hub.host = "wss://nats-server";
          nats.hub.user = "orchestrator";
          nats.hub.tlsInsecure = true;
          
          # NSC Proxy configuration
          nats.nsc_proxy = {
            enable = true;
            url = "http://nats-server:${builtins.toString nscProxyPort}";
            authKeyFile = "/var/lib/holo-orchestrator/nsc-proxy-auth.key";
          };
        };

        # Create auth key for orchestrator
        system.activationScripts.orchestrator-nsc-proxy-auth-key = ''
          mkdir -p /var/lib/holo-orchestrator
          echo "${testAuthKey}" > /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chmod 600 /var/lib/holo-orchestrator/nsc-proxy-auth.key
          chown holo-orchestrator:holo-orchestrator /var/lib/holo-orchestrator/nsc-proxy-auth.key
        '';

        # Install test dependencies
        environment.systemPackages = with pkgs; [ curl jq ];
      };
    };

    # Test script
    testScript = ''
      import subprocess
      import json
      import time

      def log(message):
          print(f"[TEST] {message}")

      def run_curl(url, method="GET", data=None, headers=None):
          cmd = ["curl", "-s", "-X", method, url]
          if data:
              cmd.extend(["-d", data])
          if headers:
              for key, value in headers.items():
                  cmd.extend(["-H", f"{key}: {value}"])
          
          result = subprocess.run(cmd, capture_output=True, text=True)
          return result.stdout, result.stderr, result.returncode

      # Wait for services to start
      log("Waiting for services to start...")
      nats_server.wait_for_unit("holo-nats-server")
      nats_server.wait_for_unit("holo-nsc-proxy")
      orchestrator.wait_for_unit("holo-orchestrator")

      # Wait for ports to be available
      log("Waiting for ports to be available...")
      nats_server.wait_for_open_port(nscProxyPort)
      nats_server.wait_for_open_port(4222)  # NATS port

      # Test 1: Health check endpoint
      log("Testing NSC proxy health endpoint...")
      health_response, _, _ = run_curl(f"http://nats-server:{nscProxyPort}/health")
      log(f"Health response: {health_response}")
      
      try:
          health_data = json.loads(health_response)
          assert health_data.get("status") == "healthy", f"Expected 'healthy', got {health_data.get('status')}"
          log("✓ Health check passed")
      except Exception as e:
          log(f"✗ Health check failed: {e}")
          raise

      # Test 2: Add user command
      log("Testing add_user command...")
      add_user_payload = {
          "command": "add_user",
          "params": {
              "account": "HPOS",
              "name": "test_user",
              "key": "test_key",
              "role": "test_role",
              "tag": "hostId:test_device"
          },
          "auth_key": "${testAuthKey}"
      }
      
      add_user_response, _, _ = run_curl(
          f"http://nats-server:{nscProxyPort}/nsc",
          method="POST",
          data=json.dumps(add_user_payload),
          headers={"Content-Type": "application/json"}
      )
      log(f"Add user response: {add_user_response}")

      # Test 3: Describe user command
      log("Testing describe_user command...")
      describe_user_payload = {
          "command": "describe_user",
          "params": {
              "account": "HPOS",
              "name": "test_user"
          },
          "auth_key": "${testAuthKey}"
      }
      
      describe_user_response, _, _ = run_curl(
          f"http://nats-server:{nscProxyPort}/nsc",
          method="POST",
          data=json.dumps(describe_user_payload),
          headers={"Content-Type": "application/json"}
      )
      log(f"Describe user response: {describe_user_response}")

      # Test 4: Invalid command (should fail)
      log("Testing invalid command (should fail)...")
      invalid_payload = {
          "command": "invalid_command",
          "params": {},
          "auth_key": "${testAuthKey}"
      }
      
      invalid_response, _, _ = run_curl(
          f"http://nats-server:{nscProxyPort}/nsc",
          method="POST",
          data=json.dumps(invalid_payload),
          headers={"Content-Type": "application/json"}
      )
      log(f"Invalid command response: {invalid_response}")
      
      try:
          invalid_data = json.loads(invalid_response)
          assert "error" in invalid_data, "Expected error in response"
          log("✓ Invalid command correctly rejected")
      except Exception as e:
          log(f"✗ Invalid command should have been rejected: {e}")
          raise

      # Test 5: Invalid auth (should fail)
      log("Testing invalid auth (should fail)...")
      invalid_auth_payload = {
          "command": "add_user",
          "params": {
              "account": "HPOS",
              "name": "test_user",
              "key": "test_key"
          },
          "auth_key": "wrong_key"
      }
      
      invalid_auth_response, _, _ = run_curl(
          f"http://nats-server:{nscProxyPort}/nsc",
          method="POST",
          data=json.dumps(invalid_auth_payload),
          headers={"Content-Type": "application/json"}
      )
      log(f"Invalid auth response: {invalid_auth_response}")
      
      try:
          invalid_auth_data = json.loads(invalid_auth_response)
          assert "error" in invalid_auth_data, "Expected error in response"
          log("✓ Invalid auth correctly rejected")
      except Exception as e:
          log(f"✗ Invalid auth should have been rejected: {e}")
          raise

      # Test 6: Orchestrator can connect to NSC proxy
      log("Testing orchestrator connection to NSC proxy...")
      
      # Wait a bit for orchestrator to fully start
      time.sleep(10)
      
      # Check if orchestrator service is healthy
      orchestrator_status = orchestrator.succeed("systemctl is-active holo-orchestrator")
      assert orchestrator_status.strip() == "active", f"Orchestrator service not active: {orchestrator_status}"
      log("✓ Orchestrator service is active")

      # Test 7: Check firewall rules (should allow orchestrator IP)
      log("Testing firewall rules...")
      firewall_rules = nats_server.succeed("iptables -L INPUT -n | grep ${builtins.toString nscProxyPort}")
      log(f"Firewall rules for port {nscProxyPort}: {firewall_rules}")
      
      # Verify that orchestrator IP is allowed
      if "10.0.0.2" in firewall_rules:
          log("✓ Firewall correctly allows orchestrator IP")
      else:
          log("✗ Firewall should allow orchestrator IP")
          raise Exception("Firewall configuration incorrect")

      log("All NSC proxy tests passed!")
    '';

    # Meta information
    meta = with lib.maintainers; {
      maintainers = [ ];
      description = "Test NSC proxy server functionality with firewall rules";
    };
  }
) 
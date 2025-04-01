# please see the `dev-` prefixed recipes in the toplevel Justfile for usage

{
  inputs,
  system,
  flake,
  perSystem,
  nixpkgs ? inputs.nixpkgs-2411,
}:

let
  privateNetwork = true;

  package = inputs.extra-container.lib.buildContainers {

    # The system of the container host
    inherit system;

    # Optional: Set nixpkgs.
    # If unset, the nixpkgs input of extra-container flake is used
    inherit nixpkgs;

    # Only set this if the `system.stateVersion` of your container
    # host is < 22.05
    # legacyInstallDirs = true;

    # Set this to disable `nix run` support
    # addRunner = false;

    config =
      { config, ... }:
      let
        hubHostAddress = "192.168.42.42";
        hubLocalAddress = "192.168.42.43";
        hostHostAddress = "192.168.42.44";
        hostLocalAddress = "192.168.42.45";
        orchestratorHostAddress = "192.168.42.46";
        orchestratorLocalAddress = "192.168.42.47";
        holoGwHostAddress = "192.168.42.48";
        holoGwLocalAddress = "192.168.42.49";
        hostMachineId = "f0b9a2b7a95848389fdb43eda8139569";

        bootstrapPort = 50000;
        signalPort = 50001;

        hosts = {
          "${hubLocalAddress}" = [ "dev-hub" ];
          "${holoGwLocalAddress}" = [ "dev-gw" ];
        };

      in
      {
        containers.dev-hub = {
          inherit privateNetwork;

          hostAddress = hubHostAddress;
          localAddress = hubLocalAddress;
          # `specialArgs` is available in nixpkgs > 22.11
          # This is useful for importing flakes from modules (see nixpkgs/lib/modules.nix).
          # specialArgs = { inherit inputs; };

          config =
            { pkgs, ... }:
            {
              networking.firewall.enable = false;

              imports = [
                flake.nixosModules.holo-nats-server
              ];

              networking.hosts = hosts;

              # networking.hostName = "hub";
              # networking.domain = "local";

              # holo.orchestrator.enable = true;
              holo.nats-server.enable = true;
              holo.nats-server.host = "0.0.0.0";
              services.nats.settings = {
                # TODO: re-enable this and replicate the same account structure on the host-agent side.
                accounts = {
                  SYS = {
                    users = [
                      {
                        user = "admin";
                        password = "admin";
                      }
                    ];
                  };
                  TESTING = {
                    jetstream = "enabled";
                    users = [
                      {
                        user = "anon";
                        password = "anon";
                      }
                      {
                        user = "host";
                        password = "$2a$11$n9GIsAxhun3dESl6QAW0feAyBjL4zdsROGyStO8EzbO5gjZg9ApOC";
                      }
                      {
                        user = "orchestrator";
                        password = "$2a$11$MhaeMYaGfTKPUphrsDHHwugySr/Z5PSEugH28ctqEYowGXiAq2eOO";
                      }

                      {
                        user = "gateway";
                        password = "$2a$11$K8SLsaSSqDw5LNxvKl64VeGm1FIJREr8Umg0qCrataTB.aAo65QHe";
                      }
                    ];
                  };
                };
                system_account = "SYS";
                # no_auth_user = "anon";

                jetstream = {
                  # TODO: use "hub" once we support different domains on hub and leafs
                  domain = "";
                  enabled = true;
                };

                # logging options
                debug = true;
                trace = false;
                logtime = false;
              };

              # TODO: create a module for this and move this to a separate container.
              systemd.services.holochain-services = {
                requires = [
                  "network-online.target"
                ];

                wantedBy = [
                  "multi-user.target"
                ];
                script = ''
                  ${pkgs.lib.getExe' perSystem.holonix_0_4.holochain "hc-run-local-services"} \
                    --bootstrap-interface="0.0.0.0" \
                    --bootstrap-port=${builtins.toString bootstrapPort} \
                    --signal-interfaces "0.0.0.0" \
                    --signal-port=${builtins.toString signalPort}
                '';
              };
            };
        };

        containers.dev-host = {
          inherit privateNetwork;
          hostAddress = hostHostAddress;
          localAddress = hostLocalAddress;

          # Forward requests from the container's external interface
          # to the container's localhost.
          # Useful to test internal services from outside the container.

          # WARNING: This exposes the container's localhost to all users.
          # Only use in a trusted environment.
          extra.exposeLocalhost = true;

          # `specialArgs` is available in nixpkgs > 22.11
          # This is useful for importing flakes from modules (see nixpkgs/lib/modules.nix).
          # specialArgs = { inherit inputs; };

          config =
            { ... }:
            {
              # in case the container shares the host network, don't mess with the firewall rules.
              networking.firewall.enable = false;

              imports = [
                flake.nixosModules.holo-host-agent
              ];

              networking.hosts = hosts;

              environment.etc."machine-id" = {
                mode = "0644";
                text = hostMachineId;
              };

              environment.systemPackages = [
                perSystem.self.rust-workspace.individual.ham
              ];

              holo.host-agent = {
                enable = true;
                logLevel = "trace";

                extraDaemonizeArgs.host-inventory-disable = false;

                # dev-container
                nats.hub.url = "wss://host:eKonohFee5ahS6pah3iu1a@dev-hub:${builtins.toString config.containers.dev-hub.config.holo.nats-server.websocket.externalPort}";
                nats.hub.tlsInsecure = true;

                # cloud testing
                # nats.hub.url = "wss://nats-server-0.holotest.dev:443";
                # nats.hub.tlsInsecure = false;

                nats.store_dir = "/var/lib/holo-host-agent/store_dir";
              };
            };
        };

        containers.dev-orch = {
          inherit privateNetwork;
          hostAddress = orchestratorHostAddress;
          localAddress = orchestratorLocalAddress;

          # `specialArgs` is available in nixpkgs > 22.11
          # This is useful for importing flakes from modules (see nixpkgs/lib/modules.nix).
          # specialArgs = { inherit inputs; };

          config =
            { pkgs, lib, ... }:
            {
              # in case the container shares the host network, don't mess with the firewall rules.
              networking.firewall.enable = false;

              imports = [
                flake.nixosModules.holo-orchestrator
              ];

              networking.hosts = hosts;

              holo.orchestrator = {
                enable = true;
                logLevel = "trace";

                nats.hub.url = "wss://dev-hub:${builtins.toString config.containers.dev-hub.config.holo.nats-server.websocket.externalPort}";
                nats.hub.tlsInsecure = true;
                nats.hub.user = "orchestrator";
                nats.hub.passwordFile = builtins.toFile "nats.pw" "yooveihuQuai4ziphiel4F";

                # TODO: actually provide an instance
                mongo.url = "mongodb://127.0.0.1";
              };

              services.mongodb = {
                enable = true;
                package = pkgs.mongodb-ce;
                bind_ip = "0.0.0.0";
              };

              nixpkgs.config.allowUnfreePredicate =
                pkg:
                builtins.elem (lib.getName pkg) [
                  "mongodb-ce"
                ];

            };
        };

        containers.dev-gw = {
          inherit privateNetwork;

          hostAddress = holoGwHostAddress;
          localAddress = holoGwLocalAddress;

          # `specialArgs` is available in nixpkgs > 22.11
          # This is useful for importing flakes from modules (see nixpkgs/lib/modules.nix).
          # specialArgs = { inherit inputs; };

          config =
            { ... }:
            {
              # in case the container shares the host network, don't mess with the firewall rules.
              networking.firewall.enable = false;

              imports = [
                flake.nixosModules.holo-gateway
              ];

              networking.hosts = hosts;

              holo.holo-gateway = {
                enable = true;
                logLevel = "trace";

                nats.hub.url = "wss://dev-hub:${builtins.toString config.containers.dev-hub.config.holo.nats-server.websocket.externalPort}";
                nats.hub.tlsInsecure = true;
                nats.hub.user = "gateway";
                nats.hub.passwordFile = builtins.toFile "nats.pw" "B_82eA5#Z7Om2J-LjqRT1B";
              };
            };
        };
      };
  };
  packageWithPlatformFilter = package.overrideAttrs {
    meta.platforms =
      with nixpkgs.lib;
      lists.intersectLists platforms.linux (platforms.x86_64 ++ platforms.aarch64);
  };

  packageWithPlatformFilterAndTest = packageWithPlatformFilter.overrideAttrs {
  };
in
packageWithPlatformFilterAndTest

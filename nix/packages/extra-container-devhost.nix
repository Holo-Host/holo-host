/*
  dev cycle
  $ just devhost-cycle
*/

{
  inputs,
  system,
  flake,
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

        devHubFqdn = config.containers.dev-hub.config.networking.fqdn;
        hosts = {
          "${hubLocalAddress}" = [ devHubFqdn ];
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
            { lib, ... }:
            {
              networking.firewall.enable = lib.mkForce false;

              imports = [
                flake.nixosModules.holo-nats-server
              ];

              networking.hostName = "hub";
              networking.domain = "local";

              # holo.orchestrator.enable = true;
              holo.nats-server.enable = true;
              services.nats.settings = {
                accounts = {
                  SYS = {
                    users = [
                      {
                        user = "admin";
                        password = "admin";
                      }
                    ];
                  };
                };
                system_account = "SYS";

                jetstream = {
                  domain = "hub";
                  enabled = true;
                };

                # logging options
                debug = true;
                trace = false;
                logtime = false;
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
            { lib, ... }:
            {
              # in case the container shares the host network, don't mess with the firewall rules.
              networking.firewall.enable = lib.mkForce false;

              imports = [
                flake.nixosModules.holo-host-agent
              ];

              networking.hosts = hosts;

              holo.host-agent = {
                enable = true;
                rust = {
                  log = "trace";
                  backtrace = "trace";
                };

                # TODO: i suspect there's a bug where the inventory prevents the workload messages from being processed
                extraDaemonizeArgs.host-inventory-disable = true;

                nats.hub.url = "wss://${devHubFqdn}:${builtins.toString config.containers.dev-hub.config.holo.nats-server.websocket.externalPort}";
                nats.hub.tlsInsecure = true;
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
            { lib, ... }:
            {
              # in case the container shares the host network, don't mess with the firewall rules.
              networking.firewall.enable = lib.mkForce false;

              imports = [
                flake.nixosModules.holo-orchestrator
              ];

              networking.hosts = hosts;

              holo.orchestrator = {
                enable = true;
                rust = {
                  log = "trace";
                  backtrace = "trace";
                };

                nats.hub.url = "wss://${devHubFqdn}:${builtins.toString config.containers.dev-hub.config.holo.nats-server.websocket.externalPort}";
                nats.hub.tlsInsecure = true;
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

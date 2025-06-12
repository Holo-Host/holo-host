/*
This file is a package that configures the container.

This can be run on a nixos machine (that has extra-containers installed ?) using:
$ nix run --extra-experimental-features "nix-command flakes" --refresh github:holo-host/holo-host#extra-container-holochain -- --restart-changed

optionally deploy locally to a dev machine:

$ nix copy --no-check-sigs "$(nix build --print-out-paths .#packages.x86_64-linux.extra-container-holochain)" --to  'ssh-ng://root@towards-allograph.dmz.internal'
*/
{
  inputs,
  system,
  flake,
  pkgs,
  nixpkgs ? inputs.nixpkgs-2411,
  privateNetwork ? false,
  index ? 0,
  adminWebsocketPort ? 8000 + index,
  containerName ? "holochain${builtins.toString index}",
  autoStart ? false,
  # these are passed to holochain
  bootstrapUrl ? null,
  signalUrl ? null,
  stunUrls ? null,
  holochainFeatures ? null,
  holochainVersion ? null,
  # hc-http-gw related args
  httpGwEnable ? false,
  httpGwAllowedAppIds ? [],
  # TODO: support
  # httpGwAllowedFns ? { },
}:
(pkgs.lib.makeOverridable (args:
  let
    package = inputs.extra-container.lib.buildContainers {
      # The system of the container host
      inherit system;

      # Optional: Set nixpkgs.
      # If unset, the nixpkgs input of extra-container flake is used
      inherit (args) nixpkgs;

      # Only set this if the `system.stateVersion` of your container
      # host is < 22.05
      # legacyInstallDirs = true;

      # Set this to disable `nix run` support
      # addRunner = false;

      config = {
        containers."${args.containerName}" = {
          inherit (args) privateNetwork autoStart;

          # `specialArgs` is available in nixpkgs > 22.11
          # This is useful for importing flakes from modules (see nixpkgs/lib/modules.nix).
          # specialArgs = { inherit inputs; };

          bindMounts."/etc/hosts" = {
            hostPath = "/etc/hosts";
            isReadOnly = true;
          };

          config = {
            lib,
            options,
            ...
          }: {
            # in case the container shares the host network, don't mess with the firewall rules.
            networking.firewall.enable = args.privateNetwork;
            networking.useHostResolvConf = true;

            imports = [
              flake.nixosModules.holochain
              flake.nixosModules.hc-http-gw
            ];

            holo.holochain =
              (
                {
                  inherit (args) adminWebsocketPort;
                  # NB: all holochain version handling logic is now located within the holochain nixos module.
                  version = args.holochainVersion;
                  features = args.holochainFeatures;
                }
              )
              // (lib.optionalAttrs (args.bootstrapUrl != null) {bootstrapServiceUrl = args.bootstrapUrl;})
              // (lib.optionalAttrs (args.signalUrl != null) {webrtcTransportPoolSignalUrl = args.signalUrl;})
              // (lib.optionalAttrs (args.stunUrls != null) {webrtcTransportPoolIceServers = args.stunUrls;})

              # TODO: add support for httpGwAllowedFns ?
              ;

            holo.hc-http-gw = {
              enable = args.httpGwEnable;
              adminWebsocketUrl = "ws://127.0.0.1:${builtins.toString args.adminWebsocketPort}";
              allowedAppIds = args.httpGwAllowedAppIds;
              # allowedFnsPerAppId = httpGwAllowedFns;
            };
          };
        };
      };

      packageWithPlatformFilter = package.overrideAttrs {
        meta.platforms = with nixpkgs.lib;
          lists.intersectLists platforms.linux (platforms.x86_64 ++ platforms.aarch64);
      };

      packageWithPlatformFilterAndTest = packageWithPlatformFilter.overrideAttrs {
        passthru.tests.integration = pkgs.testers.runNixOSTest (
          {
            nodes,
            lib,
            ...
          }: {
            name = "host-agent-integration-nixos";
            meta.platforms = lib.lists.intersectLists lib.platforms.linux lib.platforms.x86_64;

            nodes.host = {...}: {
              imports = [
                inputs.extra-container.nixosModules.default
              ];
            };

            testScript = _: ''
              host.start()
              host.wait_for_unit("multi-user.target")
              host.succeed("extra-container create ${package}")

              # ensure the port is closed before starting the holochain container
              host.wait_for_closed_port(${builtins.toString args.adminWebsocketPort}, timeout = 1)

              host.succeed("extra-container start ${args.containerName}")
              host.wait_until_succeeds("systemctl -M ${args.containerName} is-active holochain", timeout = 60)

              # now the port should be open
              host.wait_for_open_port(${builtins.toString args.adminWebsocketPort}, timeout = 1)
            '';
          }
        );
      };
    in
      packageWithPlatformFilterAndTest
)) {
  inherit
    inputs
    system
    flake
    pkgs
    nixpkgs
    privateNetwork
    index
    adminWebsocketPort
    containerName
    autoStart
    bootstrapUrl
    signalUrl
    stunUrls
    holochainFeatures
    holochainVersion
    httpGwEnable
    httpGwAllowedAppIds
    ;
}

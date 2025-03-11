/*
  this can be run on a nixos machine (that has extra-containers installed ?) using:
  $ nix run --refresh github:holo-host/holo-host/extra-container-template-and-holochain#extra-container-holochain -- --restart-changed

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
}:

let

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

    config = {
      containers."${containerName}" = {
        inherit privateNetwork;

        # `specialArgs` is available in nixpkgs > 22.11
        # This is useful for importing flakes from modules (see nixpkgs/lib/modules.nix).
        # specialArgs = { inherit inputs; };

        config =
          { ... }:
          {
            # in case the container shares the host network, don't mess with the firewall rules.
            networking.firewall.enable = privateNetwork;

            holo.holochain = {
              inherit adminWebsocketPort;
            };

            imports = [
              flake.nixosModules.holochain
            ];
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
    passthru.tests.integration = pkgs.testers.runNixOSTest (
      { nodes, lib, ... }:
      {
        name = "host-agent-integration-nixos";
        meta.platforms = lib.lists.intersectLists lib.platforms.linux lib.platforms.x86_64;

        nodes.host =
          { ... }:
          {
            imports = [
              inputs.extra-container.nixosModules.default
            ];
          };

        testScript = _: ''
          host.start()
          host.wait_for_unit("multi-user.target")
          host.succeed("extra-container create ${package}")

          # ensure the port is closed before starting the holochain container
          host.wait_for_closed_port(${builtins.toString adminWebsocketPort}, timeout = 1)

          host.succeed("extra-container start ${containerName}")
          host.wait_until_succeeds("systemctl -M ${containerName} is-active holochain", timeout = 60)

          # now the port should be open
          host.wait_for_open_port(${builtins.toString adminWebsocketPort}, timeout = 1)
        '';
      }
    );

  };
in
packageWithPlatformFilterAndTest

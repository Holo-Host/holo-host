{
  flake,
  system,
  pkgs,
  perSystem,
  ...
}:
let
  craneLib = flake.lib.mkCraneLib { inherit pkgs system; };
in
craneLib.devShell {
  inputsFrom =
    [
      flake.devShells.${system}.default
    ]
    # Inherit inputs from rust-workspace on the platforms it's available.
    ++ (pkgs.lib.lists.optionals
      (pkgs.lib.meta.availableOn pkgs.stdenv.hostPlatform flake.packages.${system}.rust-workspace)
      (
        [
          flake.packages.${system}.rust-workspace
        ]
        ++ (builtins.attrValues flake.packages.${system}.rust-workspace.passthru.tests)
      )
    );

  # Additional dev-shell environment variables can be set directly
  # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

  # Extra inputs can be added here; cargo and rustc are provided by default.
  packages =
    [
      pkgs.natscli
      perSystem.holonix_0_4.holochain
    ]
    ++ (pkgs.lib.lists.optionals pkgs.stdenv.isLinux [
      # building this takes very long, so until necessary don't build on non-linux, i.e. darwin
      perSystem.self.hc-http-gw
    ]);
}

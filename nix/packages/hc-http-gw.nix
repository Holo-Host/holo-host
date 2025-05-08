# Losely following the tutorial at https://crane.dev/examples/quick-start-workspace.html
{
  flake,
  pkgs,
  system,
  perSystem,
  ...
}: let
  craneLib = flake.lib.mkCraneLib {inherit pkgs system;};
  srcDeps = craneLib.cleanCargoSource flake.inputs.hc-http-gw;
  src = let
    # Only keeps markdown files
    markdownFilter = path: _type:
    # the build relies on this file
      path != "spec.md";
    markdownOrCargo = path: type: (markdownFilter path type) || (craneLib.filterCargoSources path type);
  in
    pkgs.lib.cleanSourceWith {
      src = flake.inputs.hc-http-gw;
      filter = markdownOrCargo;
      name = "source"; # Be reproducible, regardless of the directory name
    };

  commonArgs = {
    src = srcDeps;
    # strictDeps = true;

    # assume the rust-workspace has all dependencies required for this package as well
    nativeBuildInputs =
      [
        pkgs.go
      ]
      ++ perSystem.self.rust-workspace.nativeBuildInputs;
    inherit
      (perSystem.self.rust-workspace)
      buildInputs
      ;

    # Additional environment variables can be set directly
    # MY_CUSTOM_VAR = "some value";
    meta.platforms = pkgs.lib.platforms.linux;
  };

  # Build *just* the cargo dependencies (of the entire workspace),
  # so we can reuse all of that work (e.g. via cachix) when running in CI
  # It is *highly* recommended to use something like cargo-hakari to avoid
  # cache misses when building individual top-level-crates
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
  craneLib.buildPackage (
    pkgs.lib.attrsets.recursiveUpdate commonArgs {
      doCheck = false;
      inherit src cargoArtifacts;
    }
  )

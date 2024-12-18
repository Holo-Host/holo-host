/*
  This exposes all crates in the workspace as a single package attribute.
  It also enforces various tests.

  Losely following the tutorial at https://crane.dev/examples/quick-start-workspace.html
*/

{
  flake,
  pkgs,
  system,
  ...
}:
let
  craneLib = flake.lib.mkCraneLib { inherit pkgs system; };
  src = craneLib.cleanCargoSource flake;
  commonArgs = {
    inherit src;
    strictDeps = true;

    buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
      # Additional darwin specific inputs can be set here
      pkgs.libiconv
    ];

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
craneLib.cargoBuild (
  commonArgs
  // {
    inherit cargoArtifacts;

    # NB: we disable tests since we'll run them all via cargo-nextest
    doCheck = false;

    passthru.tests = {
      clippy = craneLib.cargoClippy (
        commonArgs
        // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        }
      );

      doc = craneLib.cargoDoc (
        commonArgs
        // {
          inherit cargoArtifacts;
        }
      );

      # Check formatting
      fmt = craneLib.cargoFmt {
        inherit src;
      };

      toml-fmt = craneLib.taploFmt {
        src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
        # taplo arguments can be further customized below as needed
        # taploExtraArgs = "--config ./taplo.toml";
      };

      # Audit licenses
      deny = craneLib.cargoDeny {
        inherit src;
      };

      nextest = craneLib.cargoNextest (
        commonArgs
        // {
          inherit cargoArtifacts;
          partitions = 1;
          partitionType = "count";
        }
      );

    };

  }
)

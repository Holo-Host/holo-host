/*
  This exposes all crates in the workspace as a single package attribute.
  It also enforces various tests.

  Losely following the tutorial at https://crane.dev/examples/quick-start-workspace.html
*/

{
  flake,
  pkgs,
  system,
  perSystem,
  ...
}:
let
  craneLib = flake.lib.mkCraneLib { inherit pkgs system; };
  src = craneLib.cleanCargoSource flake;
  commonArgs = {
    inherit src;
    strictDeps = true;

    nativeBuildInputs = [
      # perl needed for openssl on all platforms
      pkgs.perl
    ];

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
craneLib.buildPackage (
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

      # TODO: Audit licenses
      # deny = craneLib.cargoDeny {
      #   inherit src;
      # };

      nextest = craneLib.cargoNextest (
        commonArgs
        // {
          inherit cargoArtifacts;

          # this will allow some read-only (by ways of little permissions) machine introspection.
          __noChroot = true;

          nativeBuildInputs =
            [
              ## hpos-hal
              pkgs.dosfstools
              pkgs.e2fsprogs
              pkgs.coreutils
              pkgs.systemd

              # pkgs.dmidecode
              # (pkgs.writeShellScriptBin "sudo" ''
              #   exec "$@"
              # '')

              ## NATS/mongodb integration tests
              pkgs.nats-server
              pkgs.nsc

              # link only the `hc` binaries into the devshell
              (pkgs.runCommand "hc" { } ''
                mkdir -p $out/bin
                for bin in ${perSystem.holonix.holochain}/bin/hc*; do
                  ln -s $bin $out/bin/
                done
              '')
            ]
            ++ (pkgs.lib.lists.optionals (!pkgs.stdenv.isAarch64) [
              # TODO: get mongodb built for aarch64
              pkgs.mongodb
            ]);
          partitions = 1;
          partitionType = "count";
        }
      );
    };
  }
)

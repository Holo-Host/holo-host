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
      pkgs.pkg-config
    ];

    buildInputs =
      [
        pkgs.openssl.dev
        pkgs.openssl
      ]
      ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
        # Additional darwin specific inputs can be set here
        pkgs.libiconv
      ];

    # Additional environment variables can be set directly
    # MY_CUSTOM_VAR = "some value";
    IGNORE_TESTS_IN_BUILDBOT = "true";

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
    inherit cargoArtifacts;

    # NB: we disable tests since we'll run them all via cargo-nextest
    doCheck = false;

    passthru.individual =
      let
        fileSetForCrate =
          paths:
          pkgs.lib.fileset.toSource {
            root = ../../.;
            # TODO(refactor): DRY this based on the workspace Cargo.toml
            fileset = pkgs.lib.fileset.unions (
              [
                (craneLib.fileset.cargoTomlAndLock ../..)

                (craneLib.fileset.commonCargoSources ../../rust/util_libs/nats)
                (craneLib.fileset.commonCargoSources ../../rust/util_libs/db)
                (craneLib.fileset.commonCargoSources ../../rust/hpos-hal)
                (craneLib.fileset.commonCargoSources ../../rust/services/workload)
                (craneLib.fileset.commonCargoSources ../../rust/services/hpos_updates)
                (craneLib.fileset.commonCargoSources ../../rust/services/inventory)
                (craneLib.fileset.commonCargoSources ../../rust/ham)
                (craneLib.fileset.commonCargoSources ../../rust/netdiag)
              ]
              ++ paths
            );
          };

        commonCargoArtifacts = craneLib.buildDepsOnly (
          pkgs.lib.attrsets.recursiveUpdate commonArgs {
            src = fileSetForCrate [ ];
          }
        );

        mkCargoArtifacts =
          src:
          craneLib.buildDepsOnly (
            pkgs.lib.attrsets.recursiveUpdate commonArgs {
              cargoArtifacts = commonCargoArtifacts;
              inherit src;
            }
          );

        individualCrateArgs = pkgs.lib.attrsets.recursiveUpdate commonArgs {
          inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
          # NB: we disable tests since we'll run them all via cargo-nextest
          doCheck = false;
        };
      in
      {
        host_agent =
          let
            src = fileSetForCrate [
              (craneLib.fileset.commonCargoSources ../../rust/clients/host_agent)
            ];
          in
          craneLib.buildPackage (
            individualCrateArgs
            // {
              inherit src;

              pname = "host_agent";
              cargoExtraArgs = "-p host_agent";
              cargoArtifacts = mkCargoArtifacts src;
              meta.mainProgram = "host_agent";
            }
          );

        orchestrator =
          let
            src = fileSetForCrate [
              (craneLib.fileset.commonCargoSources ../../rust/clients/orchestrator)
            ];
          in
          craneLib.buildPackage (
            individualCrateArgs
            // {
              inherit src;

              pname = "orchestrator";
              cargoExtraArgs = "-p orchestrator";
              cargoArtifacts = mkCargoArtifacts src;
            }
          );

        ham =
          let
            src = fileSetForCrate [
              (craneLib.fileset.commonCargoSources ../../rust/ham)
            ];
          in
          craneLib.buildPackage (
            individualCrateArgs
            // {
              inherit src;

              pname = "ham";
              cargoExtraArgs = "-p ham";
              cargoArtifacts = mkCargoArtifacts src;
            }
          );

        holo-gateway =
          let
            src = fileSetForCrate [
              (craneLib.fileset.commonCargoSources ../../rust/holo-gateway)
            ];
          in
          craneLib.buildPackage (
            individualCrateArgs
            // {
              inherit src;

              pname = "holo-gateway";
              cargoExtraArgs = "-p holo-gateway";
              cargoArtifacts = mkCargoArtifacts src;
            }
          );
      };

    passthru.tests = {
      clippy = craneLib.cargoClippy (
        pkgs.lib.attrsets.recursiveUpdate commonArgs {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          IGNORE_TESTS_IN_BUILDBOT = "true";
        }
      );

      doc = craneLib.cargoDoc (
        pkgs.lib.attrsets.recursiveUpdate commonArgs {
          inherit cargoArtifacts;
          IGNORE_TESTS_IN_BUILDBOT = "true";
        }
      );

      # TODO: Audit licenses
      # deny = craneLib.cargoDeny {
      #   inherit src;
      # };

      nextest = craneLib.cargoNextest (
        pkgs.lib.attrsets.recursiveUpdate commonArgs {
          inherit cargoArtifacts;

          # this will allow some read-only (by ways of little permissions) machine introspection.
          __noChroot = true;

          nativeBuildInputs =
            # TODO(dry): the recursiveUpdate is supposed to merge this list with commonArgs.nativeBuildInputs, but it doesn't work.
            commonArgs.nativeBuildInputs ++ [
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
                for bin in ${perSystem.holonix_0_4.holochain}/bin/hc*; do
                  ln -s $bin $out/bin/
                done
              '')

              # MongoDB only needed for tests
              pkgs.mongodb-ce
            ];

          partitions = 1;
          partitionType = "count";
          IGNORE_TESTS_IN_BUILDBOT = "true";
        }
      );
    };
  }
)

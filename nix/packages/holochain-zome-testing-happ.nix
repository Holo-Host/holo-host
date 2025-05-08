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
}: let
  craneLib = flake.lib.mkCraneLib {inherit pkgs system;};
  src = craneLib.cleanCargoSource flake;
  commonArgs = {
    inherit src;
    strictDeps = true;

    cargoExtraArgs = "-p holochain_zome_testing_0_integrity -p holochain_zome_testing_0";
    CARGO_BUILD_TARGET = "wasm32-unknown-unknown";

    buildInputs = [];

    # Additional environment variables can be set directly
    # MY_CUSTOM_VAR = "some value";
  };

  # Build *just* the cargo dependencies (of the entire workspace),
  # so we can reuse all of that work (e.g. via cachix) when running in CI
  # It is *highly* recommended to use something like cargo-hakari to avoid
  # cache misses when building individual top-level-crates
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  wasmBuild = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;

      # NB: we disable tests since we'll run them all via cargo-nextest
      doCheck = false;

      passthru.tests = {
      };
    }
  );

  dnas = let
    dnaYaml = pkgs.writeText "dna.yaml" ''
      manifest_version: '1'
      name: holochain_zome_testing_0_integrity
      integrity:
        network_seed: null
        origin_time: 1735841273312901
        zomes:
        - name: holochain_zome_testing_0_integrity
          hash: null
          bundled: '${wasmBuild}/lib/holochain_zome_testing_0_integrity.wasm'
      coordinator:
        zomes:
        - name: holochain_zome_testing_0
          hash: null
          bundled: '${wasmBuild}/lib/holochain_zome_testing_0.wasm'
          dependencies:
          - name: holochain_zome_testing_0_integrity
    '';
  in
    pkgs.runCommand "dnas"
    {
      nativeBuildInputs = [
        perSystem.holonix_0_4.holochain
        pkgs.tree
      ];
    }
    ''
      tree ${wasmBuild}
      mkdir dnas
      cp ${dnaYaml} dnas/dna.yaml
      cd dnas
      cat dna.yaml
      hc dna pack .

      mkdir $out
      cp -r * $out/
    '';

  happ = let
    happYaml = pkgs.writeText "happ.yaml" ''
      ---
      manifest_version: "1"
      name: test-happ-0
      description: "holochain integrity zome for testing 0"
      roles:
        - name: holochain_zome_testing_0_integrity
          provisioning:
            strategy: create
            deferred: false
          dna:
            bundled: ./dnas/holochain_zome_testing_0_integrity.dna
            modifiers:
              properties: ~
              network_seed: ~
    '';
  in
    pkgs.runCommand "happ"
    {
      nativeBuildInputs = [
        perSystem.holonix_0_4.holochain
        pkgs.tree
      ];

      meta.platforms = pkgs.lib.platforms.linux;
    }
    ''
      mkdir $out

      cp -r ${dnas} $out/dnas

      cp ${happYaml} $out/happ.yaml
      cd $out

      tree
      cat happ.yaml
      hc app pack . -o $out/happ.bundle
    '';
in
  happ

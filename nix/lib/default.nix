{ inputs, flake, ... }:

{
  mkCraneLib =
    { pkgs, system }:
    let
      craneLib = inputs.crane.mkLib pkgs;
      toolchain = (inputs.rust-overlay.lib.mkRustBin { } pkgs).fromRustupToolchainFile (
        flake + "/rust-toolchain.toml"
      );
    in
    craneLib.overrideToolchain toolchain;

  runNixOSTest' =
    { pkgs, system }:

    /*
      looks like this:
      args: { fn body that has access to args}
      or this:
      { static attrs }
    */
    callerArg:

    let
      callerFn = pkgs.lib.toFunction callerArg;
    in

    pkgs.testers.runNixOSTest (
      args:
      (pkgs.lib.recursiveUpdate {
        defaults._module.args = {
          inherit flake inputs system;
        };
      } (callerFn args))
    );
}

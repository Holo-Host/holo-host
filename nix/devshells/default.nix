{
  pkgs,
  flake,
  system,
}:
pkgs.mkShell {
  # Add build dependencies
  packages = [ flake.formatter.${system} ];

  # Add environment variables
  env = { };

  # Load custom bash code
  shellHook =
    # TODO(blocked/upstream): remove this once https://github.com/isbecker/treefmt-vscode/issues/3 is resolved
    (flake.inputs.nixago.lib.${system}.make {
      data = flake.formatter.${system}.settings;
      output = "treefmt.toml";
      format = "toml";
    }).shellHook
    + ''
      echo $(git rev-parse --show-toplevel)
    '';
}

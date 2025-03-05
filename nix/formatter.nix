{
  pkgs,
  inputs,
  perSystem,
  ...
}:
let
  settingsNix = {
    package = perSystem.nixpkgs.treefmt2;

    projectRootFile = ".git/config";

    programs = {
      nixfmt.enable = true;
      deadnix = {
        enable = true;
        no-underscore = true;
      };
      statix.enable = true;

      rustfmt.enable = true;

      gofmt.enable = true;

      shfmt.enable = true;

      prettier.enable = true;

      taplo.enable = true;
    } // pkgs.lib.optionalAttrs (pkgs.system != "riscv64-linux") { shellcheck.enable = true; };

    settings = {
      global.excludes = [
        "LICENSE"
        # unsupported extensions
        "*.{gif,png,svg,tape,mts,lock,mod,sum,env,envrc,gitignore}"
      ];

      formatter = {
        deadnix = {
          priority = 1;
        };

        nixfmt = {
          priority = 2;
        };

        statix = {
          priority = 3;
        };

        prettier = {
          options = [
            "--tab-width"
            "2"
          ];
          includes = [ "*.{css,html,js,json,jsx,md,mdx,scss,ts,yaml}" ];
        };
      };
    };
  };

  treefmtEval = inputs.treefmt-nix.lib.evalModule pkgs settingsNix;

in
treefmtEval.config.build.wrapper.overrideAttrs (_: {
  passthru = {
    inherit (treefmtEval.config) package settings;
    inherit (treefmtEval) config;
    inherit settingsNix;
  };
})

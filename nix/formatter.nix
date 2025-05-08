{
  pkgs,
  inputs,
  perSystem,
  ...
}: let
  settingsNix = {
    package = perSystem.nixpkgs.treefmt2;
    projectRootFile = ".git/config";

    # Enable formatters for different file types
    programs =
      {
        # Nix formatting
        alejandra.enable = true;
        deadnix = {
          enable = true;
          no-underscore = true;
        };
        statix.enable = true;

        # Rust formatting
        rustfmt.enable = true;

        # Shell formatting and linting
        shfmt.enable = true;

        # Web formatting
        prettier.enable = true;

        # Other formatters
        gofmt.enable = true;
        taplo.enable = true;
      }
      // pkgs.lib.optionalAttrs (pkgs.system != "riscv64-linux") {
        shellcheck.enable = true;
      };

    # Global settings
    settings = {
      # Files to exclude from formatting
      global.excludes = [
        "LICENSE"
        # Unsupported extensions
        "*.{gif,png,svg,tape,mts,lock,mod,sum,env,envrc,gitignore}"
      ];

      # Formatter-specific settings
      formatter = {
        # Nix formatters
        alejandra = {
          priority = 1; # Run alejandra first for Nix files
        };
        deadnix = {
          priority = 2; # Run deadnix after alejandra
        };
        statix = {
          priority = 3; # Run statix last for Nix files
        };

        # Web formatters
        prettier = {
          priority = 1; # Run prettier first for web files
          options = ["--tab-width" "2"];
          includes = ["*.{css,html,js,json,jsx,md,mdx,scss,ts,yaml}"];
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

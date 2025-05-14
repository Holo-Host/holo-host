{
  pkgs,
  flake,
  system,
}:
pkgs.mkShell {
  # Add build dependencies
  packages = [
    flake.formatter.${system}
    pkgs.jq
    pkgs.just
    pkgs.mongosh
    pkgs.systemd
    pkgs.util-linux # for journalctl deps
    # Add extra-container for development container management
    flake.inputs.extra-container.packages.${system}.default
    # Add additional systemd-related packages
    pkgs.systemd-container
    pkgs.machinectl
  ];

  # Add environment variables
  env = {
    # Ensure systemd can find its configuration
    SYSTEMD_NSPAWN_TMPFS_TMP = "1";
  };

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
      
      # Check if running in a systemd environment
      if ! systemctl is-system-running --quiet; then
        echo "Warning: Not running in a systemd environment. Some container features may not work."
      fi
    '';
}

# Module to configure a machine as a holo-agent.
{
  lib,
  config,
  pkgs,
  system,
  flake,
  ...
}:

let
  cfg = config.holo.agent;
in
{
  imports = [
    # TODO: this causes an infinite recursion. we do need this to run workloads.
    # flake.inputs.extra-container.nixosModules.default
  ];

  options.holo.agent = {
    enable = lib.mkOption {
      description = "enable holo-agent";
      default = true;
    };

    autoStart = lib.mkOption {
      default = true;
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = flake.packages.${system}.rust-workspace;
    };

    rust = {
      log = lib.mkOption {
        type = lib.types.str;
        default = "debug";
      };

      backtrace = lib.mkOption {
        type = lib.types.str;
        default = "1";
      };
    };

    nats = {
      useOsNats = lib.mkOption {
        type = lib.types.bool;
        default = true;
      };

      url = lib.mkOption {
        type = lib.types.str;
      };
      hubServerUrl = lib.mkOption {
        type = lib.types.str;
      };
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.holo-agent = {
      enable = true;

      requires = lib.lists.optional cfg.nats.useOsNats "nats.service";
      after = lib.lists.optional cfg.nats.useOsNats "nats.service";

      requiredBy = lib.optional cfg.autoStart "multi-user.target";

      environment = {
        RUST_LOG = cfg.rust.log;
        RUST_BACKTRACE = cfg.rust.backtrace;
        NATS_URL = cfg.nats.url;
        NATS_HUB_SERVER_URL = cfg.nats.hubServerUrl;
      };

      script = builtins.toString (
        pkgs.writeShellScript "holo-agent" ''
          ${lib.getExe' cfg.package "host_agent"} daemonize
        ''
      );
    };

    # TODO: add nats server here or is it started by the agent?
  };
}

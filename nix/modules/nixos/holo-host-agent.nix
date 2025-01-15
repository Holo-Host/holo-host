# Module to configure a machine as a holo-host-agent.

# blueprint specific first level argument that's referred to as "publisherArgs"
{
  inputs,
  ...
}:

{
  lib,
  config,
  pkgs,
  ...
}:

let
  cfg = config.holo.host-agent;
in
{
  imports = [
    inputs.extra-container.nixosModules.default
  ];

  options.holo.host-agent = {
    enable = lib.mkOption {
      description = "enable holo-host-agent";
      default = true;
    };

    autoStart = lib.mkOption {
      default = true;
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = inputs.self.packages.${pkgs.stdenv.system}.rust-workspace;
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
        default = false;
      };

      listenHost = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
      };

      listenPort = lib.mkOption {
        type = lib.types.int;
        default = 4222;
      };

      url = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = "${cfg.nats.listenHost}:${builtins.toString cfg.nats.listenPort}";
      };

      hubServerUrl = lib.mkOption {
        type = lib.types.str;
      };
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.holo-host-agent = {
      enable = true;

      requires = lib.lists.optional cfg.nats.useOsNats "nats.service";
      after = lib.lists.optional cfg.nats.useOsNats "nats.service";

      requiredBy = lib.optional cfg.autoStart "multi-user.target";

      environment =
        {
          RUST_LOG = cfg.rust.log;
          RUST_BACKTRACE = cfg.rust.backtrace;
          NATS_HUB_SERVER_URL = cfg.nats.hubServerUrl;
          NATS_LISTEN_PORT = builtins.toString cfg.nats.listenPort;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.url != null) {
          NATS_URL = cfg.nats.url;
        };

      path = [
        pkgs.nats-server
      ];

      script = builtins.toString (
        pkgs.writeShellScript "holo-host-agent" ''
          ${lib.getExe' cfg.package "host_agent"} daemonize
        ''
      );
    };

    # TODO: add nats server here or is it started by the host-agent?
  };
}

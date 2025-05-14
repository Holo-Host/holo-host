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
      default = inputs.self.packages.${pkgs.stdenv.system}.rust-workspace.passthru.individual.host_agent;
    };

    logLevel = lib.mkOption {
      type = lib.types.str;
      default = "debug";
    };

    rust = {
      log = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
      };

      backtrace = lib.mkOption {
        type = lib.types.str;
        default = "1";
      };
    };

    nats = {
      listenHost = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
      };

      listenPort = lib.mkOption {
        type = lib.types.int;
        default = 4222;
      };

      url = lib.mkOption {
        type = lib.types.str;
        default = "${cfg.nats.listenHost}:${builtins.toString cfg.nats.listenPort}";
      };

      hub = {
        url = lib.mkOption {
          type = lib.types.str;
        };
        tlsInsecure = lib.mkOption {
          type = lib.types.bool;
        };
      };

      store_dir = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
      };
    };

    extraDaemonizeArgs = lib.mkOption {
      # forcing everything to be a string because the bool -> str conversion is strange (true -> "1" and false -> "")
      type = lib.types.attrs;
      default = {
      };
    };

  };

  config = lib.mkIf cfg.enable {
    systemd.services.holo-host-agent = {
      enable = true;

      after = [
        "network-online.target"
      ];
      wants = [
        "network-online.target"
      ];
      wantedBy = lib.lists.optional cfg.autoStart "multi-user.target";

      environment =
        {
          RUST_LOG =
            if cfg.rust.log != null then
              cfg.rust.log
            else
              "${cfg.logLevel},request=error,tungstenite=error,async_nats=error,mio=error,tokio_tungstenite=error";
          RUST_BACKTRACE = cfg.rust.backtrace;
          NATS_LISTEN_PORT = builtins.toString cfg.nats.listenPort;
          NIX_REMOTE = "daemon";
        }
        // lib.attrsets.optionalAttrs (cfg.nats.url != null) {
          NATS_URL = cfg.nats.url;
        };

      path = config.environment.systemPackages ++ [
        pkgs.git
        pkgs.nats-server
        pkgs.natscli
        pkgs.nsc
      ];

      script =
        let
          extraDaemonizeArgsList = lib.attrsets.mapAttrsToList (
            name: value:
            if lib.types.bool.check value then
              (lib.optionalString value "--${name}")
            else if (value == lib.types.int.check value || lib.types.path.check value) then
              "--${name}=${builtins.toString value}"
            else if (lib.types.str.check value) then
              "--${name}=${value}"
            else
              throw "${name}: don't know how to handle type ${value}"
          ) cfg.extraDaemonizeArgs;
        in
        builtins.toString (
          pkgs.writeShellScript "holo-host-agent" ''
            ${lib.getExe' cfg.package "host_agent"} daemonize \
              --hub-url=${cfg.nats.hub.url} \
              ${lib.optionalString cfg.nats.hub.tlsInsecure "--hub-tls-insecure"} \
              ${lib.optionalString (cfg.nats.store_dir != null) "--store-dir=${cfg.nats.store_dir}"} \
              ${builtins.concatStringsSep " " extraDaemonizeArgsList}
          ''
        );
    };
  };
}

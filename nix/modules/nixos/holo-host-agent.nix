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

      nscPath = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/.local/share/nats/nsc";
      };

      sharedCredsPath = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.nats.nscPath}/shared_creds";
      };

      localCredsPath = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.nats.nscPath}/local_creds";
      };

      hostNkeyPath = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.nats.localCredsPath}/host.nk";
      };

      sysNkeyPath = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.nats.localCredsPath}/sys.nk";
      };

      hposCredsPath = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/holo-host-agent/server-key-config.json";
      };

      hposCredsPw = lib.mkOption {
        type = lib.types.path;
        default = "pass";
      };

      hub = {
        url = lib.mkOption {
          type = lib.types.str;
        };
        tlsInsecure = lib.mkOption {
          type = lib.types.bool;
        };
      };

      extraDaemonizeArgs = lib.mkOption {
        # forcing everything to be a string because the bool -> str conversion is strange (true -> "1" and false -> "")
        type = lib.types.attrs;
        default = {
        };
      };
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.holo-host-agent = {
      enable = true;

      after = [
        "network.target"
        "network-online.target"
      ];
      wantedBy = lib.lists.optional cfg.autoStart "multi-user.target";

      environment =
        {
          RUST_LOG = cfg.rust.log;
          RUST_BACKTRACE = cfg.rust.backtrace;
          NSC_PATH = cfg.nats.nscPath;
          LOCAL_CREDS_PATH = cfg.nats.localCredsPath;
          HOSTING_AGENT_HOST_NKEY_PATH = cfg.nats.hostNkeyPath;
          HOSTING_AGENT_SYS_NKEY_PATH = cfg.nats.sysNkeyPath;
          HPOS_CONFIG_PATH = cfg.nats.hposCredsPath;
          DEVICE_SEED_DEFAULT_PASSWORD = builtins.toString cfg.nats.hposCredsPw;
          NATS_LISTEN_PORT = builtins.toString cfg.nats.listenPort;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.url != null) {
          NATS_URL = cfg.nats.url;
        };

      path = [
        pkgs.nats-server
      ];

      preStart = ''
              echo "Start Host Auth Setup"
        mkdir -p ${cfg.nats.hostNkeyPath}
        mkdir -p ${cfg.nats.sysNkeyPath}
        mkdir -p ${cfg.nats.hposCredsPath}
        echo "Finshed Host Auth Setup"
      '';

      script =
        let
          extraDaemonizeArgsList = lib.attrsets.mapAttrsToList (
            name: value:
            let
              type = lib.typeOf value;
            in
            if type == lib.types.str then
              "--${name}=${value}"
            else if (type == lib.types.int || type == lib.types.path) then
              "--${name}=${builtins.toString value}"
            else if type == lib.types.bool then
              (lib.optionalString value name)
            else
              throw "don't know how to handle type ${type}"
          ) cfg.nats.extraDaemonizeArgs;
        in
        builtins.toString (
          pkgs.writeShellScript "holo-host-agent" ''
            ${lib.getExe' cfg.package "host_agent"} daemonize \
              --hub-url=${cfg.nats.hub.url} \
              ${lib.optionalString cfg.nats.hub.tlsInsecure "--hub-tls-insecure"} \
              ${builtins.concatStringsSep " " extraDaemonizeArgsList}
          ''
        );
    };
  };
}

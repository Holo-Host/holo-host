# Module to configure a machine as a holo-orchestrator.

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
  cfg = config.holo.orchestrator;
in
{
  imports = [
    inputs.extra-container.nixosModules.default
  ];

  options.holo.orchestrator = {
    enable = lib.mkOption {
      description = "enable holo-orchestrator";
      default = true;
    };

    autoStart = lib.mkOption {
      type = lib.types.bool;
      default = true;
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = inputs.self.packages.${pkgs.stdenv.system}.rust-workspace;
    };

    hubAuthScriptPath =  lib.mkOption {
      type = lib.types.path;
      default = "${cfg.package}/../../../scripts/hub_auth_setup";
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

    mongo = {
      documentation.enable = false;

      listenHost = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
      };

      listenPort = lib.mkOption {
        type = lib.types.int;
        default = 27017;
      };

      url = lib.mkOption {
        type = lib.types.str;
        default = "${cfg.mongo.listenHost}:${builtins.toString cfg.mongo.listenPort}";
      };

      services.mongodb = {
        enable = true;
        bind_ip = cfg.mongo.listenHost;
      };

      systemd.services.mongodb.serviceConfig = {
        LimitNOFILE = 500000;
      };

      networking.firewall.allowedTCPPorts = [ cfg.mongo.listenPort ];
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

      tlsInsecure = lib.mkOption {
        type = lib.types.bool;
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

      rootAuthNkeyPath = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.nats.localCredsPath}/AUTH_ROOT_SK.nk";
      };

      signingAuthNkeyPath = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.nats.localCredsPath}/AUTH_SK.nk";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.holo-orchestrator = {
      enable = true;

      after = [
        "network.target"
      ];
      wantedBy = lib.lists.optional cfg.autoStart "multi-user.target";

      environment =
        {
          RUST_LOG = cfg.rust.log;
          RUST_BACKTRACE = cfg.rust.backtrace;
          MONGO_URI = cfg.mongo.url;
          NSC_PATH = cfg.nats.nscPath;
          LOCAL_CREDS_PATH = cfg.nats.localCredsPath;
          ORCHESTRATOR_ROOT_AUTH_NKEY_PATH = cfg.nats.rootAuthNkeyPath;
          ORCHESTRATOR_SIGNING_AUTH_NKEY_PATH = cfg.nats.signingAuthNkeyPath;
          NATS_LISTEN_PORT = builtins.toString cfg.nats.listenPort;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.url != null) {
          NATS_URL = cfg.nats.url;
        };

      path = [
        pkgs.nats-server
        pkgs.bash
      ];

      preStart = ''
        init_hub_auth() {
          ${cfg.hubAuthScriptPath} ${cfg.nats.listenHost} ${builtins.toString cfg.nats.listenPort} ${builtins.toString cfg.nats.sharedCredsPath} ${builtins.toString cfg.nats.localCredsPath}
        }
        init_hub_auth
        echo "Finshed Hub Auth Setup"
        sleep 1 # wait
      '';

      script = ''
        ${lib.getExe' cfg.package "orchestrator"}
      '';
    };
  };
}

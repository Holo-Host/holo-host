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
      default = inputs.self.packages.${pkgs.stdenv.system}.rust-workspace.individual.orchestrator;
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

    mongo = {
      username = lib.mkOption {
        type = lib.types.str;
        default = "orchestrator";
      };

      clusterIdFile = lib.mkOption {
        type = lib.types.str;
        default = "/var/lib/config/mongo/cluster_id.txt";
        description = "Path to the file containing the Orchestrator MongoDB cluster id";
      };

      passwordFile = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/config/mongo/password.txt";
        description = "Path to the file containing the Orchestrator MongoDB password";
      };
    };

    nats = {
      hub = {
        url = lib.mkOption {
          type = lib.types.str;
        };

        tlsInsecure = lib.mkOption {
          type = lib.types.bool;
        };

        user = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
        };

        passwordFile = lib.mkOption {
          type = lib.types.nullOr lib.types.path;
          default = null;
        };
      };
    };

  };

  config = lib.mkIf cfg.enable {
    systemd.services.holo-orchestrator = {
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
              "${cfg.logLevel},tungstenite=error,async_nats=error,mio=error";

          RUST_BACKTRACE = cfg.rust.backtrace;
          MONGODB_USERNAME = cfg.mongo.username;
          MONGODB_CLUSTER_ID_FILE = cfg.mongo.clusterIdFile;
          MONGODB_PASSWORD_FILE = cfg.mongo.passwordFile;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.hub.url != null) {
          NATS_URL = cfg.nats.hub.url;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.hub.user != null) {
          NATS_USER = cfg.nats.hub.user;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.hub.passwordFile != null) {
          NATS_PASSWORD_FILE = "%d/NATS_PASSWORD_FILE";
        }
        // lib.attrsets.optionalAttrs cfg.nats.hub.tlsInsecure {
          NATS_SKIP_TLS_VERIFICATION_DANGER = "true";
        };

      serviceConfig.LoadCredential = lib.lists.optional (cfg.nats.hub.passwordFile != null) [
        # specified like: <filename inside unit>:<source of secret>
        "NATS_PASSWORD_FILE:${cfg.nats.hub.passwordFile}"

        # Using agenix, perhaps:
        #
        # "target:${config.age.secrets.greeting_target.path}"
      ];

      path = [
        pkgs.nats-server
        pkgs.bash
      ];

      script = ''
        ${lib.getExe' cfg.package "orchestrator"}
      '';
    };
  };
}

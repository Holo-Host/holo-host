# Module to configure a machine as a holo-orchestrator.
# This is the main orchestrator module that imports all service-specific modules
{inputs, ...}: {
  lib,
  config,
  pkgs,
  ...
}: let
  cfg = config.holo.orchestrator;
in {
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
      description = "The orchestrator package to use";
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
      server = {
        host = lib.mkOption {
          type = lib.types.str;
          default = "127.0.0.1";
        };
        port = lib.mkOption {
          type = lib.types.int;
          default = 4222;
        };

        url = lib.mkOption {
          type = lib.types.str;
          default = "nats://${config.holo.orchestrator.nats.server.host}:${builtins.toString config.holo.orchestrator.nats.server.port}";
        };

        tlsInsecure = lib.mkOption {
          type = lib.types.bool;
          default = false;
        };
        
        user = lib.mkOption {
          type = lib.types.nullOr lib.types.str;
          default = null;
        };

        /* TODO: remove `passwordFile` once we confirm the succesful use of nsc */
        passwordFile = lib.mkOption {
          type = lib.types.nullOr lib.types.path;
          default = null;
        };
      };

      nsc_proxy = {
        enable = lib.mkOption {
          type = lib.types.bool;
          default = true;
          description = "Enable NSC proxy integration";
        };

        url = lib.mkOption {
          type = lib.types.str;
          default = "http://nats-server-0.holotest.dev:5000";
          description = "URL of the NSC proxy server";
        };

        authKeyFile = lib.mkOption {
          type = lib.types.path;
          description = "Path to file containing the NSC proxy authentication key";
        };

        natsServerHost = lib.mkOption {
          type = lib.types.str;
          default = "nats-server-0.holotest.dev";
          description = "Hostname of the NATS server for public key distribution";
        };

        natsServerUser = lib.mkOption {
          type = lib.types.str;
          default = "root";
          description = "Username for SSH access to NATS server";
        };

        natsServerPubkeyPath = lib.mkOption {
          type = lib.types.str;
          default = "/var/lib/nats_server/orchestrator_auth_pubkey.txt";
          description = "Path on NATS server where public key should be stored";
        };
      };

      nsc = {
        # NSC configuration
        path = lib.mkOption {
          type = lib.types.nullOr lib.types.path;
          default = null;
        };
        
        # Credential paths (local path where credentials are extracted)
        credsPath = lib.mkOption {
          type = lib.types.path;
          description = "Local path where NATS credentials are extracted and stored";
        };

        # Credential file names
        adminCredsFile = lib.mkOption {
          type = lib.types.str;
          default = "${config.holo.orchestrator.nats.nsc.credsPath}/admin.creds";
          description = "Admin user credentials file name";
        };

        authCredsFile = lib.mkOption {
          type = lib.types.str;
          default = "${config.holo.orchestrator.nats.nsc.credsPath}/orchestrator_auth.creds";
          description = "Auth user credentials file name";
        };

        rootAuthNkeyPath = lib.mkOption {
          type = lib.types.path;
          default = "${config.holo.orchestrator.nats.nsc.credsPath}/AUTH_ROOT_SK.nk";
        };

        signingAuthNkeyPath = lib.mkOption {
          type = lib.types.path;
          default = "${config.holo.orchestrator.nats.nsc.credsPath}/AUTH_SK.nk";
        };
      };
    };
  };

  config = lib.mkIf cfg.enable {
    # Create orchestrator user
    users.users.orchestrator = {
      isSystemUser = true;
      home = "/var/lib/orchestrator";
      createHome = true;
    };

    # Main holo-orchestrator service configuration
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
            if cfg.rust.log != null
            then cfg.rust.log
            else "${cfg.logLevel},tungstenite=error,async_nats=error,mio=error";

          RUST_BACKTRACE = cfg.rust.backtrace;
          MONGODB_USERNAME = cfg.mongo.username;
          MONGODB_CLUSTER_ID_FILE = cfg.mongo.clusterIdFile;
          MONGODB_PASSWORD_FILE = cfg.mongo.passwordFile;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.server.url != null) {
          NATS_URL = cfg.nats.server.url;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.server.user != null) {
          NATS_USER = cfg.nats.server.user;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.nsc.path != null) {
          NSC_PATH = "%d/NSC_PATH";
          ORCHESTRATOR_ROOT_AUTH_NKEY_PATH = "${cfg.nats.nsc.rootAuthNkeyPath}";
          ORCHESTRATOR_SIGNING_AUTH_NKEY_PATH = "${cfg.nats.nsc.signingAuthNkeyPath}";
          NATS_ADMIN_CREDS_FILE = "admin.creds";
          NATS_AUTH_CREDS_FILE = "orchestrator_auth.creds";
        }
        // lib.attrsets.optionalAttrs (cfg.nats.server.passwordFile != null) {
          NATS_PASSWORD_FILE = "%d/NATS_PASSWORD_FILE";
        }
        // lib.attrsets.optionalAttrs cfg.nats.nsc_proxy.enable {
          NSC_PROXY_URL = "${cfg.nats.nsc_proxy.url}";
        }
        // lib.attrsets.optionalAttrs cfg.nats.server.tlsInsecure {
          NATS_SKIP_TLS_VERIFICATION_DANGER = "true";
        };

      serviceConfig = {
        # Restart policy
        Restart = "always";
        RestartSec = "10";
        StartLimitInterval = "120";
        StartLimitBurst = "3";
        
        # Security settings
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        
        # File system access
        ReadWritePaths = [
          "/var/lib/orchestrator"
          cfg.nats.nsc.credsPath
        ];
        
        User = "orchestrator";
        
        LoadCredential =
        lib.lists.optional (cfg.nats.server.passwordFile != null) [
          # specified like: <filename inside unit>:<source of secret>
          "NATS_PASSWORD_FILE:${cfg.nats.server.passwordFile}"

          # Using agenix, perhaps:
          #
          # "target:${config.age.secrets.greeting_target.path}"
        ]
        ++ [
          "MONGODB_CLUSTER_ID_FILE:${cfg.mongo.clusterIdFile}"
          "MONGODB_PASSWORD_FILE:${cfg.mongo.passwordFile}"
        ]
        ++ lib.lists.optional cfg.nats.nsc_proxy.enable [
          "NSC_PROXY_AUTH_KEY:${cfg.nats.nsc_proxy.authKeyFile}"
        ];
      };

      path = [
        pkgs.nats-server
        pkgs.bash
        pkgs.nsc
        pkgs.netcat
      ];

      script = ''
        ${lib.getExe' cfg.package "orchestrator"}
      '';
    };
  };
} 
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
      bind_ip = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
      };

      url = lib.mkOption {
        type = lib.types.str;
        default = "mongodb://${cfg.mongo.bind_ip}";

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
      };
    };

  };

  config = lib.mkIf cfg.enable {
    services.ferretdb = {
      enable = true;
      settings.listen-addr = "127.0.0.1:27017";
      # inherit (cfg.mongo) bind_ip;
    };

    # virtualisation.docker.rootless = {
    #   enable = true;
    #   setSocketVariable = true;
    # };

    # virtualisation.oci-containers.backend = "docker";
    # virtualisation.oci-containers.containers = {
    #   container-name = {
    #     image = "docker.io/library/mongo:latest";
    #     autoStart = true;
    #     ports = [ "127.0.0.1:27017:27017" ];
    #   };
    # };

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
          RUST_LOG = cfg.rust.log;
          RUST_BACKTRACE = cfg.rust.backtrace;
          MONGO_URI = cfg.mongo.url;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.hub.url != null) {
          NATS_URL = cfg.nats.hub.url;
        };

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

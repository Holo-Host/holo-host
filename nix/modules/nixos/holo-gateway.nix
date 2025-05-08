# Module to configure a machine as a holo-gateway.
# blueprint specific first level argument that's referred to as "publisherArgs"
{inputs, ...}: {
  lib,
  config,
  pkgs,
  ...
}: let
  cfg = config.holo.holo-gateway;
in {
  options.holo.holo-gateway = {
    enable = lib.mkOption {
      description = "enable holo-gateway";
      default = true;
    };

    autoStart = lib.mkOption {
      type = lib.types.bool;
      default = true;
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = inputs.self.packages.${pkgs.stdenv.system}.rust-workspace.individual.holo-gateway;
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

    address = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
    };

    port = lib.mkOption {
      type = lib.types.int;
      default = 8000;
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

    caddy = {
      enable = lib.mkOption {
        description = "enable caddy reverse-proxy";
        default = true;
      };
      staging = lib.mkOption {
        type = lib.types.bool;
        description = "use staging acmeCA for testing purposes. change this in production enviornments.";
        default = true;
      };
      logLevel = lib.mkOption {
        type = lib.types.str;
        default = "DEBUG";
      };
    };

    extraAttrs = lib.mkOption {
      description = "extra attributes passed to `services.nats`";
      default = {};
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.holo-gateway = {
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

          LISTEN = "${cfg.address}:${builtins.toString cfg.port}";
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
        pkgs.bash
      ];

      script = ''
        ${lib.getExe' cfg.package "holo-gateway"}
      '';
    };

    networking.firewall.allowedTCPPorts =
      # need port 80 to receive well-known ACME requests
      [cfg.port]
      ++ (lib.optionals cfg.caddy.enable [
        80
        443
      ]);

    services.caddy = lib.mkIf cfg.caddy.enable (
      {
        enable = true;
        globalConfig = ''
          auto_https disable_redirects
        '';
        logFormat = ''
          level ${cfg.caddy.logLevel}
        '';

        virtualHosts = let
          maybe_fqdn = builtins.tryEval config.networking.fqdn;
          domain =
            if maybe_fqdn.success
            then maybe_fqdn.value
            else builtins.trace "WARNING: FQDN is not available, this will most likely lead to an invalid caddy configuration. falling back to hostname ${config.networking.hostName}" config.networking.hostName;
        in {
          "http://${domain}:80".extraConfig = ''
            tls {
              issuer acme
              issuer internal
            }
            reverse_proxy http://127.0.0.1:${builtins.toString cfg.port}
          '';

          "https://${domain}".extraConfig = ''
            tls {
              issuer acme
              issuer internal
            }
            reverse_proxy http://127.0.0.1:${builtins.toString cfg.port}
          '';
        };
      }
      // lib.attrsets.optionalAttrs cfg.caddy.staging {
        acmeCA = "https://acme-staging-v02.api.letsencrypt.org/directory";
      }
    );
  };
}

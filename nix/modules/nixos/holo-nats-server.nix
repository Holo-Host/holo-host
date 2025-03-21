/*
  Opinionated module to configure a NATS server to be used with other holo-host components.
  The main use-case for this module will be to host a NATS cluster that is reachable by all hosts.
*/
{ lib, config, ... }:
let
  cfg = config.holo.nats-server;
in
{
  imports = [ ];

  options.holo.nats-server = {
    enable = lib.mkOption {
      description = "enable holo NATS server";
      default = true;
    };

    host = lib.mkOption {
      description = "native client listen host";
      type = lib.types.str;
      default = "127.0.0.1";
    };

    port = lib.mkOption {
      description = "native client port";
      type = lib.types.int;
      default = 4222;
    };

    leafnodePort = lib.mkOption {
      description = "native leafnode port";
      type = lib.types.int;
      default = 7422;
    };

    websocket = {
      port = lib.mkOption {
        description = "websocket listen port";
        type = lib.types.int;
        default = 4223;
      };

      externalPort = lib.mkOption {
        description = "expected external websocket port";
        type = lib.types.nullOr lib.types.int;
        default = 443;
      };

      openFirewall = lib.mkOption {
        description = "allow incoming TCP connections to the externalWebsocket port";
        default = false;
      };
    };

    caddy = {
      enable = lib.mkOption {
        description = "enable holo NATS server";
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
      default = { };
    };
  };

  config = lib.mkIf cfg.enable {
    networking.firewall.allowedTCPPorts =
      # need port 80 to receive well-known ACME requests
      lib.optional cfg.caddy.enable 80
      ++ lib.optional (cfg.websocket.openFirewall || cfg.caddy.enable) cfg.websocket.externalPort;

    services.nats = lib.mkMerge [
      {
        serverName = lib.mkDefault config.networking.hostName;
        enable = lib.mkDefault true;
        jetstream = lib.mkDefault true;

        settings = {
          host = lib.mkDefault cfg.host;
          port = lib.mkDefault cfg.port;
          leafnodes.port = lib.mkDefault cfg.leafnodePort;
          websocket = {
            inherit (cfg.websocket) port;

            # TLS will be terminated by the reverse-proxy
            no_tls = true;
          };
        };
      }
      cfg.extraAttrs
    ];

    services.caddy = lib.mkIf cfg.caddy.enable (
      {
        enable = true;
        globalConfig = ''
          auto_https disable_redirects
        '';
        logFormat = ''
          level DEBUG
        '';

        virtualHosts =
          let
            maybe_fqdn = builtins.tryEval config.networking.fqdn;
            domain =
              if maybe_fqdn.success then
                maybe_fqdn.value
              else
                builtins.trace "WARNING: FQDN is not available, this will most likely lead to an invalid caddy configuration. falling back to hostname ${config.networking.hostName}" config.networking.hostName;
          in
          {
            "https://${domain}:${builtins.toString cfg.websocket.externalPort}".extraConfig = ''
              tls {
                issuer acme
                issuer internal
              }
              reverse_proxy http://127.0.0.1:${builtins.toString cfg.websocket.port}
            '';
          };
      }
      // lib.attrsets.optionalAttrs cfg.caddy.staging {
        acmeCA = "https://acme-staging-v02.api.letsencrypt.org/directory";
      }
    );
  };
}

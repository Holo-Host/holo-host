
# Module to configure a machine as a holo-nats-server.
# This is the main nats-server module that imports all service-specific modules
{inputs, ...}: {
  lib,
  config,
  pkgs,
  ...
}: let
  cfg = config.holo.nats-server;
in {
  options.holo.nats-server = {
    enable = lib.mkOption {
      description = "Enable NATS Server";
      type = lib.types.bool;
      default = true;
    };

    # NATS Server configuration
    server = {
      host = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
        description = "NATS Server hostname";
      };

      port = lib.mkOption {
        type = lib.types.int;
        default = 4222;
        description = "NATS Server port";
      };
    };

    # WebSocket configuration
    websocket = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Enable WebSocket support";
      };

      port = lib.mkOption {
        type = lib.types.int;
        default = 443;
        description = "WebSocket port";
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

    # JetStream configuration
    jetstream = {
      domain = lib.mkOption {
        description = "jetstream domain name";
        type = lib.types.str;
        default = "holo";
      };
      enabled = lib.mkOption {
        description = "enable jetstream in nats server";
        type = lib.types.bool;
        default = true;
      };
    };

    # Caddy configuration for TLS termination
    caddy = {
      enable = lib.mkOption {
        description = "enable caddy reverse-proxy for WebSocket TLS termination";
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

    # NSC configuration
    nsc = {
      # NSC configuration path
      path = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/nats_server/.local/share/nats/nsc";
        description = "Path to NSC configuration directory";
      };

      # Local credentials path
      localCredsPath = lib.mkOption {
        type = lib.types.path;
        description = "Path for local credentials (signing keys)";
      };

      # Shared credentials path
      sharedCredsPath = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/nats_server/shared-creds";
        description = "Path for shared JWT files";
      };

      # Resolver configuration path
      resolverPath = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/nats_server/main-resolver.conf";
        description = "Path to the NATS resolver configuration file";
      };
    };

    # NATS Server configuration file
    configFile = lib.mkOption {
      type = lib.types.path;
      default = pkgs.writeText "nats-server.conf" ''
        # NATS Server configuration
        port: ${builtins.toString cfg.server.port}
        http_port: 8222
        server_name: nats-server
        
        # JWT authentication
        operator: ${cfg.nsc.sharedCredsPath}/HOLO.jwt
        resolver: ${cfg.nsc.resolverPath}
        system_account: SYS
        
        # Logging
        logtime: true
        debug: false
        trace: false
        
        # Clustering
        cluster {
          port: 6222
          listen: 0.0.0.0:6222
        }
        
        # JetStream configuration
        jetstream {
          domain: ${cfg.jetstream.domain}
          enabled: ${lib.boolToString cfg.jetstream.enabled}
        }
        
        # WebSocket configuration
        ${lib.optionalString cfg.websocket.enable ''
        websocket {
          port: ${builtins.toString cfg.websocket.port}
          no_tls: true
        }
        ''}
      '';
      description = "NATS Server configuration file";
    };
  };

  config = lib.mkIf cfg.enable {
    # Firewall configuration for Caddy
    networking.firewall.allowedTCPPorts =
      # need port 80 to receive well-known ACME requests
      lib.optional cfg.caddy.enable 80
      ++ lib.optional (cfg.websocket.openFirewall || cfg.caddy.enable) cfg.websocket.externalPort;

    # Create nats server user and group
    users.groups.nats-server = {};
    users.users.nats-server = {
      isSystemUser = true;
      home = "/var/lib/nats_server";
      createHome = true;
      group = "nats-server";
    };

    # Create necessary directories
    system.activationScripts.holo-nats-server-dirs = ''
      mkdir -p ${cfg.nsc.path}
      mkdir -p /var/lib/nats_server
      chown -R nats-server:nats-server /var/lib/nats_server
      chmod -R 700 /var/lib/nats_server
    '';

    # NATS Server service
    systemd.services.nats = {
      description = "NATS Server";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      path = [ pkgs.nats-server ];

      script = ''
        # Check if JWT credentials exist before starting NATS
        if [ ! -f "${cfg.nsc.sharedCredsPath}/HOLO.jwt" ]; then
          echo "ERROR: NATS JWT credentials not found. Please ensure setup has completed."
          exit 1
        fi
        
        exec ${pkgs.nats-server}/bin/nats-server -c ${cfg.configFile}
      '';

      serviceConfig = {
        Type = "simple";
        Restart = "always";
        RestartSec = "1";
        
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
          "/var/lib/nats_server"
          cfg.nsc.sharedCredsPath
          cfg.nsc.localCredsPath
          cfg.nsc.path
        ];
        
        User = "nats-server";
        Group = "nats-server";
      };
    };

    # Caddy service for TLS termination
    services.caddy = lib.mkIf cfg.caddy.enable (
      {
        enable = true;
        globalConfig = ''
          auto_https disable_redirects
        '';
        logFormat = ''
          level ${cfg.caddy.logLevel}
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

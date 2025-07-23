
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

    workingDirectory = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/nats_server";
      description = "Working directory for NATS Server";
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
        type = lib.types.nullOr lib.types.int;
        default = 443;
        description = "expected external websocket port";
      };

      openFirewall = lib.mkOption {
        default = false;
        description = "allow incoming TCP connections to the externalWebsocket port";
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
        default = "${cfg.workingDirectory}/.local/share/nats/nsc";
        # "/root/.local/share/nats/nsc"
        description = "Path to NSC configuration directory";
      };

      # Local credentials path
      localCredsPath = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.workingDirectory}/nsc/local-creds";
        description = "Path for local credentials (signing keys)";
      };

      # Shared credentials path
      sharedCredsPath = lib.mkOption {
        type = lib.types.path;
        default = "${cfg.workingDirectory}/shared-creds";
        description = "Path for shared JWT files";
      };

      # Resolver configuration path
      resolverFileName = lib.mkOption {
        type = lib.types.str;
        default = "main-resolver.conf";
        description = "Path to the NATS resolver configuration file";
      };
    };

    # Enable JWT authentication
    enableJwt = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Enable JWT authentication for NATS server";
    };

    extraAttrs = lib.mkOption {
      description = "extra attributes passed to `services.nats`";
      default = { };
    };

    # NATS Server configuration file
    configFile = lib.mkOption {
      type = lib.types.path;
      default = pkgs.writeText "nats-server.conf" ''
        # NATS Server configuration
        port: ${builtins.toString cfg.server.port}
        http_port: 8222
        server_name: nats-server
        
        ${lib.optionalString cfg.enableJwt ''
        # JWT authentication
        operator: ${cfg.nsc.sharedCredsPath}/HOLO.jwt
        system_account: SYS
        ''}
        include "${cfg.nsc.resolverFileName}"
        
        # Logging
        logtime: true
        debug: false
        trace: false

        # JetStream configuration
        jetstream {
          domain: ${cfg.jetstream.domain}
          enabled: ${lib.boolToString cfg.jetstream.enabled}
        }
                
        # Clustering - disabled for single-node setup
        # cluster {
        #   port: 6222
        #   listen: 0.0.0.0:6222
        # }
        
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
    # Set XDG_DATA_HOME for all services in this module
    environment.variables.XDG_DATA_HOME = "${cfg.workingDirectory}/.data";
    
    # Firewall configuration for Caddy
    networking.firewall.allowedTCPPorts =
      # need port 80 to receive well-known ACME requests
      lib.optional cfg.caddy.enable 80
      ++ lib.optional (cfg.websocket.openFirewall || cfg.caddy.enable) cfg.websocket.externalPort;

    # Create nats server user and group
    users.groups.nats-server ={};
    users.users.nats-server = {
      isSystemUser = true;
      home = "${cfg.workingDirectory}";
      createHome = true;
      group = "nats-server";
    };

    # Create necessary directories
    system.activationScripts.holo-nats-server-dirs = ''
      ${lib.optionalString cfg.enableJwt "mkdir -p ${cfg.nsc.path}"}
      mkdir -p ${cfg.workingDirectory}
      chown -R nats-server:nats-server ${cfg.workingDirectory}
      chmod -R 700 ${cfg.workingDirectory}

      # Ensure resolver config exists
      if [ ! -f "${cfg.workingDirectory}/${cfg.nsc.resolverFileName}" ]; then
        echo 'resolver: MEMORY' > "${cfg.workingDirectory}/${cfg.nsc.resolverFileName}"
        chown nats-server:nats-server "${cfg.workingDirectory}/${cfg.nsc.resolverFileName}"
        chmod 600 "${cfg.workingDirectory}/${cfg.nsc.resolverFileName}"
      fi

      # Copy config file from Nix store to "${cfg.workingDirectory}/nats-server.conf"
      cp ${cfg.configFile} "${cfg.workingDirectory}/nats-server.conf"
      chown nats-server:nats-server "${cfg.workingDirectory}/nats-server.conf"
      chmod 600 "${cfg.workingDirectory}/nats-server.conf"
    '';

    # NATS Server service
    systemd.services.nats = {
      description = "NATS Server";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ] ++ lib.optional cfg.enableJwt "holo-nats-auth-setup.service";
      requires = [ "network-online.target" ] ++ lib.optional cfg.enableJwt "holo-nats-auth-setup.service";

      path = [ pkgs.nats-server ];

      script = ''
        ${lib.optionalString cfg.enableJwt ''
        # Check if JWT credentials exist before starting NATS
        if [ ! -f "${cfg.nsc.sharedCredsPath}/HOLO.jwt" ]; then
          echo "ERROR: NATS JWT credentials not found. Please ensure setup has completed."
          exit 1
        fi
        
        # Check if resolver configuration exists before starting NATS
        if [ ! -f "${cfg.workingDirectory}/${cfg.nsc.resolverFileName}" ]; then
          echo "ERROR: NATS resolver configuration not found. Please ensure auth setup has completed."
          exit 1
        fi
        ''}
        exec ${pkgs.nats-server}/bin/nats-server -c ${cfg.workingDirectory}/nats-server.conf
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
          "${cfg.workingDirectory}"
        ] ++ lib.optional cfg.enableJwt cfg.nsc.sharedCredsPath
          ++ lib.optional cfg.enableJwt cfg.nsc.localCredsPath
          ++ lib.optional cfg.enableJwt cfg.nsc.path
          ++ lib.optional cfg.enableJwt (builtins.dirOf "${cfg.workingDirectory}/${cfg.nsc.resolverFileName}");
        
        User = "nats-server";
        Group = "nats-server";
        WorkingDirectory = "${cfg.workingDirectory}";
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
                builtins.trace "WARNING: FQDN is not available, this will most likely lead to an invalid caddy configuration... Falling back to hostname ${config.networking.hostName}" config.networking.hostName;
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

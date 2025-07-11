# Module to configure the NSC Proxy Server service.
# This service runs on the NATS Server machine to provide secure remote NSC access.
{ inputs, ... }: {
  lib,
  config,
  pkgs,
  ...
}: let
  cfg = config.holo.nsc-proxy;
in {
  options.holo.nsc-proxy = {
    enable = lib.mkOption {
      description = "Enable NSC Proxy Server service";
      type = lib.types.bool;
      default = false;
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = inputs.self.packages.${pkgs.stdenv.system}.rust-workspace.individual.nsc-proxy-server;
      description = "Package containing the NSC proxy server";
    };

    server = {
      host = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
        description = "Host to bind to";
      };

      port = lib.mkOption {
        type = lib.types.int;
        default = 5000;
        description = "Port to bind to";
      };
    };

    auth = {
      keyFile = lib.mkOption {
        type = lib.types.path;
        description = "Path to file containing the authentication key";
      };
    };

    nsc = {
      path = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/nats_server/.local/share/nats/nsc";
        description = "Path to NSC configuration directory";
      };
    };

    firewall = {
      allowedIPs = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [];
        description = "List of IP addresses allowed to access the proxy";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    # Create nsc-proxy user
    users.users.nsc-proxy = {
      isSystemUser = true;
      home = "/var/lib/nsc-proxy";
      createHome = true;
    };

    # Create necessary directories
    system.activationScripts.holo-nsc-proxy-dirs = ''
      mkdir -p /var/lib/nsc-proxy
      chown nsc-proxy:nsc-proxy /var/lib/nsc-proxy
      chmod 700 /var/lib/nsc-proxy
    '';

    # NSC Proxy Server service
    systemd.services.holo-nsc-proxy = {
      description = "NSC Proxy Server for secure remote NSC access";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      path = [
        pkgs.nsc
        pkgs.bash
      ];

      environment = {
        NSC_PATH = cfg.nsc.path;
      };

      script = ''
        ${lib.getExe cfg.package} \
          --host ${cfg.server.host} \
          --port ${builtins.toString cfg.server.port} \
          --auth-key "$(cat ${cfg.auth.keyFile})" \
          --nsc-path ${cfg.nsc.path}
      '';

      serviceConfig = {
        Type = "simple";
        Restart = "always";
        RestartSec = "10";
        StartLimitInterval = "120";
        StartLimitBurst = "3";
        
        # Security settings
        User = "nsc-proxy";
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
          cfg.nsc.path
          "/var/lib/nsc-proxy"
        ];
        
        # Load authentication key
        LoadCredential = "auth_key:${cfg.auth.keyFile}";
      };
    };

    # Firewall configuration
    networking.firewall = lib.mkIf (cfg.firewall.allowedIPs != []) {
      allowedTCPPorts = [ cfg.server.port ];
      extraCommands = ''
        # Allow access only from specified IPs
        ${lib.concatStringsSep "\n" (map (ip: "iptables -A INPUT -p tcp --dport ${builtins.toString cfg.server.port} -s ${ip} -j ACCEPT") cfg.firewall.allowedIPs)}
        # Drop all other connections to the proxy port
        iptables -A INPUT -p tcp --dport ${builtins.toString cfg.server.port} -j DROP
      '';
    };
  };
} 
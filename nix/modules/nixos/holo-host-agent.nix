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
  rootPath = "/var/lib/holo-host-agent/";
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
      type = lib.types.bool;
      default = true;
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = inputs.self.packages.${pkgs.stdenv.system}.rust-workspace.passthru.individual.host_agent;
    };

    hostAuthScriptPath = lib.mkOption {
      type = lib.types.path;
      default = inputs.self.packages.${pkgs.stdenv.system}.host-agent-auth-setup;
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

    nats = {
      host = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
      };

      listenPort = lib.mkOption {
        type = lib.types.int;
        default = 4222;
      };

      url = lib.mkOption {
        type = lib.types.str;
        default = "${cfg.nats.host}:${builtins.toString cfg.nats.listenPort}";
      };

      nsc = {
        path = lib.mkOption {
          type = lib.types.path;
          default = "/root/.local/share/nats/nsc";
        };

        sharedCredsPath = lib.mkOption {
          type = lib.types.path;
          default = "${rootPath}/store_dir/shared_creds";
        };
        
        localCredsPath = lib.mkOption {
          type = lib.types.path;
          default = "${rootPath}/store_dir/local_creds";
        };

        hostNkeyPath = lib.mkOption {
          type = lib.types.path;
          default = "${cfg.nats.nsc.localCredsPath}/host.nk";
        };

        sysNkeyPath = lib.mkOption {
          type = lib.types.path;
          default = "${cfg.nats.nsc.localCredsPath}/sys.nk";
        };

        resolverPath = lib.mkOption {
          type = lib.types.path;
          default = "/root/.local/share/nats/nsc/main-resolver.conf";
        };
      };

      # Note: Old SCP-based options removed in favor of secure Nix store approach
      # Use sharedCredsSource and sharedCredsHash for secure credential distribution

      # Secure Nix store approach
      sharedCredsSource = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        description = "Path to a derivation that produces the shared credentials (more secure than SCP)";
        default = null;
      };

      sharedCredsHash = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        description = "Expected SHA256 hash of the shared credentials for integrity verification";
        default = null;
      };

      hosterCredsPath = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/holo-host-agent/server-key-config.json";
      };

      hosterCredsPwFile = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/holo-host-agent/hpos_creds_pw.txt";
      };

      hub = {
        url = lib.mkOption {
          type = lib.types.str;
        };
        tlsInsecure = lib.mkOption {
          type = lib.types.bool;
        };
      };

      store_dir = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
      };
    };

    extraDaemonizeArgs = lib.mkOption {
      # forcing everything to be a string because the bool -> str conversion is strange (true -> "1" and false -> "")
      type = lib.types.attrs;
      default = {
      };
    };

    containerPrivateNetwork = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Input flag to determine whether to use private networking. When true, containers are isolated with port forwarding. When false, containers share the host network with dynamic port allocation to avoid conflicts.";
    };

  };

  config = lib.mkIf cfg.enable {
    # Add required packages for socat port forwarding when using private networking
    environment.systemPackages = with pkgs; [
      git
      nats-server
      natscli
      nsc
    ] ++ lib.optionals cfg.containerPrivateNetwork [
      # Additional packages needed for socat port forwarding with private networking
      socat
      netcat-gnu
      iproute2
    ];

    # Remove the insecure SCP service and replace with secure Nix store approach
    systemd.services.holo-host-agent-setup-shared-creds = lib.mkIf (cfg.nats.sharedCredsSource != null) {
      description = "Setup shared creds directory for holo-host-agent from Nix store";
      wantedBy = [ "holo-host-agent.service" ];
      before = [ "holo-host-agent.service" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        Type = "oneshot";
        User = "root";
        ProtectSystem = "strict";
        ProtectHome = true;
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
      };
      script = ''
        set -e
        echo "Setting up shared creds from Nix store derivation"
        
        # Clean existing directory
        rm -rf ${cfg.nats.nsc.sharedCredsPath}
        mkdir -p ${cfg.nats.nsc.sharedCredsPath}
        
        # Copy from Nix store derivation
        cp -r ${cfg.nats.sharedCredsSource}/* ${cfg.nats.nsc.sharedCredsPath}/
        
        # Verify integrity if hash is provided
        if [ -n "${cfg.nats.sharedCredsHash}" ]; then
          echo "Verifying shared creds integrity..."
          ACTUAL_HASH=$(find ${cfg.nats.nsc.sharedCredsPath} -type f -exec sha256sum {} \; | sort | sha256sum | cut -d' ' -f1)
          if [ "$ACTUAL_HASH" != "${cfg.nats.sharedCredsHash}" ]; then
            echo "ERROR: Shared creds integrity check failed!"
            echo "Expected: ${cfg.nats.sharedCredsHash}"
            echo "Actual: $ACTUAL_HASH"
            exit 1
          fi
          echo "Shared creds integrity verified successfully"
        fi
        
        # Set secure permissions
        chmod -R 600 ${cfg.nats.nsc.sharedCredsPath}/*
        chown -R holo-host-agent:holo-host-agent ${cfg.nats.nsc.sharedCredsPath}
        
        # Create and secure localCredsPath directory
        mkdir -p ${cfg.nats.nsc.localCredsPath}
        chmod 700 ${cfg.nats.nsc.localCredsPath}
        chown holo-host-agent:holo-host-agent ${cfg.nats.nsc.localCredsPath}
        
        echo "Shared creds setup completed successfully"
      '';
    };

    systemd.services.holo-host-agent = {
      enable = true;
      after = [
        "network-online.target"
        "holo-host-agent-setup-shared-creds.service"
      ];
      wants = [
        "network-online.target"
        "holo-host-agent-setup-shared-creds.service"
      ];
      wantedBy = lib.lists.optional cfg.autoStart "multi-user.target";

      environment =
        {
          RUST_LOG =
            if cfg.rust.log != null then
              cfg.rust.log
            else
              "${cfg.logLevel},request=error,tungstenite=error,async_nats=error,mio=error,tokio_tungstenite=error";
          RUST_BACKTRACE = cfg.rust.backtrace;
          NSC_PATH = cfg.nats.nsc.path;
          LOCAL_CREDS_PATH = cfg.nats.nsc.localCredsPath;
          HOSTING_AGENT_HOST_NKEY_PATH = cfg.nats.nsc.hostNkeyPath;
          HOSTING_AGENT_SYS_NKEY_PATH = cfg.nats.nsc.sysNkeyPath;
          HPOS_CONFIG_PATH = cfg.nats.hosterCredsPath;
          DEVICE_SEED_DEFAULT_PASSWORD_FILE = builtins.toString cfg.nats.hosterCredsPwFile;
          NIX_REMOTE = "daemon";
          IS_CONTAINER_ON_PRIVATE_NETWORK = builtins.toString cfg.containerPrivateNetwork;
          # NATS_LISTEN_PORT = builtins.toString cfg.nats.listenPort;
        }
        // lib.attrsets.optionalAttrs (cfg.nats.url != null) {
          NATS_URL = cfg.nats.url;
        };

      path = config.environment.systemPackages ++ [
        pkgs.git
        pkgs.nats-server
        pkgs.natscli
        pkgs.nsc
      ];

      preStart = ''
        # Function definition moved to top
        init_host_auth_guard() {
          ${cfg.hostAuthScriptPath} ${builtins.toString cfg.nats.nsc.path} ${builtins.toString cfg.nats.nsc.sharedCredsPath}
          echo "Finished Host Auth Guard Setup"
        }

        echo "Starting Host Auth Setup"
        if [ ! -d ${cfg.nats.nsc.sharedCredsPath} ]; then
          echo "ERROR: Shared creds directory ${cfg.nats.nsc.sharedCredsPath} does not exist!"
          exit 1
        fi
        mkdir -p ${cfg.nats.nsc.localCredsPath}
        mkdir -p ${cfg.nats.nsc.hostNkeyPath}
        mkdir -p ${cfg.nats.nsc.sysNkeyPath}
        mkdir -p ${cfg.nats.hosterCredsPath}

        init_host_auth_guard

        echo "Finished Host Auth Setup"
      '';

      script =
        let
          extraDaemonizeArgsList = lib.attrsets.mapAttrsToList (
            name: value:
            if lib.types.bool.check value then
              (lib.optionalString value "--${name}")
            else if (value == lib.types.int.check value || lib.types.path.check value) then
              "--${name}=${builtins.toString value}"
            else if (lib.types.str.check value) then
              "--${name}=${value}"
            else
              throw "${name}: don't know how to handle type ${value}"
          ) cfg.extraDaemonizeArgs;
        in
        builtins.toString (
          pkgs.writeShellScript "holo-host-agent" ''
            ${lib.getExe' cfg.package "host_agent"} daemonize \
              --hub-url=${cfg.nats.hub.url} \
              ${lib.optionalString cfg.nats.hub.tlsInsecure "--hub-tls-insecure"} \
              ${lib.optionalString (cfg.nats.store_dir != null) "--store-dir=${cfg.nats.store_dir}"} \
              ${builtins.concatStringsSep " " extraDaemonizeArgsList}
          ''
        );
    };
  };
}

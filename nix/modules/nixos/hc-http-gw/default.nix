# Module to configure a hc-http-gw service on a nixos machine.
# blueprint specific first level argument that's referred to as "publisherArgs"
{flake, ...}: {
  lib,
  config,
  pkgs,
  ...
}: let
  cfg = config.holo.hc-http-gw;
in {
  options.holo.hc-http-gw = {
    enable = lib.mkOption {
      description = "enable hc-http-gw";
      default = true;
    };

    autoStart = lib.mkOption {
      default = true;
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = flake.packages.${pkgs.system}.hc-http-gw;
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

    user = lib.mkOption {
      default = "hc-http-gw";
    };
    group = lib.mkOption {
      default = "hc-http-gw";
    };

    adminWebsocketUrl = lib.mkOption {
      type = lib.types.str;
    };

    listenAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
    };

    listenPort = lib.mkOption {
      type = lib.types.int;
      default = 8090;
    };

    allowedAppIds = lib.mkOption {
      description = "list of installed app ids that are allowed to be used by the gateway.";
      type = lib.types.listOf lib.types.str;
      default = [];
    };

    allowedFnsPerAppId = lib.mkOption {
      description = "allowed functions given per app id";
      type = lib.types.attrs;
      default = pkgs.lib.genAttrs cfg.allowedAppIds (_: "*");
    };
  };

  config = lib.mkIf cfg.enable {
    users.groups.${cfg.group} = {};
    users.users.${cfg.user} = {
      isSystemUser = true;
      inherit (cfg) group;
    };

    systemd.services.hc-http-gw = {
      enable = true;

      after = [
        "network.target"
        "holochain.service"
      ];
      wants = [
        "network.target"
        "holochain.service"
      ];
      wantedBy = lib.lists.optional cfg.autoStart "multi-user.target";

      restartIfChanged = true;

      environment =
        {
          RUST_LOG = "${cfg.rust.log},wasmer_compiler_cranelift=warn";
          RUST_BACKTRACE = cfg.rust.backtrace;

          HC_GW_ADMIN_WS_URL = cfg.adminWebsocketUrl;
          HC_GW_ADDRESS = cfg.listenAddress;
          HC_GW_PORT = builtins.toString cfg.listenPort;
          HC_GW_ALLOWED_APP_IDS = builtins.concatStringsSep "," cfg.allowedAppIds;
        }
        // (
          # add the required prefix in front of each appId
          lib.mapAttrs' (
            appId: allowedFns: (lib.nameValuePair "HC_GW_ALLOWED_FNS_${appId}" allowedFns)
          )
          cfg.allowedFnsPerAppId
        );

      serviceConfig = let
        StateDirectory = "hc-http-gw";
      in {
        User = cfg.user;
        Group = cfg.user;
        # %S is short for StateDirectory
        WorkingDirectory = "%S/${StateDirectory}";
        inherit StateDirectory;
        StateDirectoryMode = "0700";
        Restart = "always";
        RestartSec = 1;
        Type = "simple"; # The hc-http-gw does *not* send a notify signal to systemd when it is ready
        NotifyAccess = "all";
      };

      script = builtins.toString (
        pkgs.writeShellScript "hc-http-gw-wrapper" ''
          set -xeE

          ${lib.getExe' cfg.package "hc-http-gw"}
        ''
      );
    };
  };
}

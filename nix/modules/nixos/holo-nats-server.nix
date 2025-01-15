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

    port = lib.mkOption {
      description = "enable holo NATS server";
      type = lib.types.int;
      default = 4222;
    };

    leafnodePort = lib.mkOption {
      description = "enable holo NATS server";
      type = lib.types.int;
      default = 7422;
    };
  };

  config = lib.mkIf cfg.enable {
    networking.firewall.allowedTCPPorts = [
      config.services.nats.port
      config.services.nats.settings.leafnodes.port
    ];

    services.nats = {
      serverName = lib.mkDefault config.networking.hostName;
      enable = true;
      jetstream = true;

      settings = {
        inherit (cfg) port;
        leafnodes.port = cfg.leafnodePort;
      };
    };
  };
}

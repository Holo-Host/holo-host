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
      enable = lib.mkDefault true;
      jetstream = lib.mkDefault true;

      settings = {
        port = lib.mkDefault cfg.port;
        leafnodes.port = lib.mkDefault cfg.leafnodePort;
      };
    };
  };
}

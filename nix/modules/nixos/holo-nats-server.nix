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

    openFirewall = lib.mkOption {
      default = false;
    };
  };

  config = lib.mkIf cfg.enable {
    networking.firewall.allowedTCPPorts = lib.optionals cfg.openFirewall [
      cfg.port
      cfg.leafnodePort
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

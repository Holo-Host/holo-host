{ lib, config, ... }:
let
  cfg = config.holo.nats-server;
in
{
  imports = [ ];

  options.holo.nats-server = with lib; {
    enable = mkOption {
      description = "enable holo NATS server";
      default = true;
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
        leafnodes.port = 7422;
      };
    };
  };
}

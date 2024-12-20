/*
  Module to configure a machine as a holo-agent.
*/

{ inputs, lib, config, ... }:

let
  cfg = config.holo.nats-server;
in
{
  imports = [
    inputs.extra-container.nixosModules.default
  ];

  options.holo.agent = with lib; {
    enable = mkOption {
      description = "enable holo-agent";
      default = true;
    };
  };

  config = lib.mkIf cfg.enable {
    # TODO: add holo-agent systemd service
    # TODO: add nats client here or is it started by the agent?
  };
}

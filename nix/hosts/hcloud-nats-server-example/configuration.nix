{ flake, ... }:
{
  imports = [
    flake.nixosModules.hardware-hetzner-cloud-cpx
    flake.nixosModules.holo-nats-server
  ];

  system.stateVersion = "24.11";
}

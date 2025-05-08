{flake, ...}: {
  imports = [
    flake.nixosModules.hardware-hetzner-cloud-cpx
  ];

  system.stateVersion = "24.11";
}

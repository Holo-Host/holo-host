{ inputs, ... }:
{
  imports = [
    inputs.srvos.nixosModules.server
    "${inputs.nixpkgs}/nixos/modules/virtualisation/qemu-vm.nix"
  ];

  system.switch.enableNg = true;
  virtualisation.vmVariant.virtualisation.graphics = false;

  users.users.support = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
    initialPassword = "support";
  };

  environment.systemPackages = [ ];

  nixpkgs.hostPlatform = "x86_64-linux";
  # nixpkgs.hostPlatform = "aarch64-linux";

  system.stateVersion = "24.05";
}

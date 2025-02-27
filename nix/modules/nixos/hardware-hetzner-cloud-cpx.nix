# This is an opinionated module to configure Hetzner Cloud CPX instances.
{ flake, ... }:
{ lib, ... }:
{
  imports = [
    flake.nixosModules.hardware-hetzner-cloud
  ];

  boot.loader.systemd-boot.enable = false;
  boot.loader.grub.efiSupport = false;

  # forcing seems required or else there's an error about duplicated devices
  boot.loader.grub.devices = lib.mkForce [ "/dev/sda" ];

  disko.devices.disk.sda = {
    device = "/dev/sda";
    type = "disk";
    content = {
      type = "gpt";
      partitions = {
        boot = {
          size = "1M";
          type = "EF02"; # for grub MBR
        };
        root = {
          size = "100%";
          content = {
            type = "btrfs";
            extraArgs = [ "-f" ]; # Override existing partition
            subvolumes = {
              # Subvolume name is different from mountpoint
              "/rootfs" = {
                mountpoint = "/";
              };
              "/nix" = {
                mountOptions = [ "noatime" ];
                mountpoint = "/nix";
              };
              "/boot" = {
                mountpoint = "/boot";
              };
            };
          };
        };
      };
    };
  };

  nixpkgs.hostPlatform = "x86_64-linux";
}

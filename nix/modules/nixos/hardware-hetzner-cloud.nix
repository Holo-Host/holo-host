# This is an opinionated module to configure Hetzner Cloud instances.
{inputs, ...}: {lib, ...}: {
  imports = [
    inputs.srvos.nixosModules.server
    inputs.srvos.nixosModules.mixins-terminfo
    inputs.srvos.nixosModules.hardware-hetzner-cloud
    inputs.disko.nixosModules.disko
  ];

  services.cloud-init = {
    settings = {
      cloud_init_modules = [
        "migrator"
        "seed_random"
        "bootcmd"
        "write-files"
        "update_hostname"
        "resolv_conf"
        "ca-certs"
        "rsyslog"
        "users-groups"

        ## these cause issues on the cpx
        # "growpart"
        # "resizefs"
        # these are not desired in a nixos environment
        # "rsyslog"
        # "rightscale_userdata"
        # "phone-home"
      ];
    };
  };

  # assumption: rely on cloud-init to receive login information if desired
  users.allowNoPasswordLogin = lib.mkDefault true;

  system.switch.enableNg = true;
  virtualisation.vmVariant.virtualisation.graphics = false;

  environment.systemPackages = [];
  system.stateVersion = "24.11";
}

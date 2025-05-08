/*
this can be run on a nixos machine (that has extra-containers installed ?) using:
$ nix run --refresh github:holo-host/holo-host/hivello-package#extra-container-hivello -- --restart-changed

it exposes the following services on the host interfaces:
* SSH - port TCP 2200 - inherits authorized keys from the host
* VNC - port TCP 5900 - unauthenticated
*/
{
  flake,
  inputs,
  system,
}: let
  nixpkgs = inputs.nixpkgs-2405;

  privateNetwork = false;
in
  (inputs.extra-container.lib.buildContainers {
    # The system of the container host
    inherit system;

    # Optional: Set nixpkgs.
    # If unset, the nixpkgs input of extra-container flake is used
    inherit nixpkgs;

    # Only set this if the `system.stateVersion` of your container
    # host is < 22.05
    # legacyInstallDirs = true;

    # Set this to disable `nix run` support
    # addRunner = false;

    config = {
      containers.demo = {
        inherit privateNetwork;

        # `specialArgs` is available in nixpkgs > 22.11
        # This is useful for importing flakes from modules (see nixpkgs/lib/modules.nix).
        # specialArgs = { inherit inputs; };

        bindMounts."/etc/ssh/authorized_keys.d/root" = {
          isReadOnly = true;
        };
        bindMounts."/etc/ssh/authorized_keys.d/dev" = {
          isReadOnly = true;
          hostPath = "/etc/ssh/authorized_keys.d/root";
        };

        bindMounts."/dev/dri/renderD128" = {
          isReadOnly = false;
        };
        bindMounts."/dev/dri/card0 " = {
          isReadOnly = false;
        };
        bindMounts."/dev/udmabuf" = {
          isReadOnly = false;
        };

        allowedDevices = [
          {
            node = "/dev/dri/renderD128";
            modifier = "rw";
          }
          {
            node = "/dev/dri/card0";
            modifier = "rw";
          }
          {
            node = "/dev/udmabuf";
            modifier = "rw";
          }
        ];

        # required by podman
        enableTun = true;

        additionalCapabilities = [
          # TODO: i saw ptrace used in the strace, not sure if it's a requirement for the happy path
          "CAP_SYS_PTRACE"
        ];

        config = {
          config,
          pkgs,
          lib,
          ...
        }: {
          # in case the container shares the host network, don't mess with the firewall rules.
          networking.firewall.enable = privateNetwork;

          users.users.dev = {
            isNormalUser = true;
            home = "/home/dev";
            extraGroups = [
              "users"
              "podman"

              "video"
              "render"

              # TODO: shouldn't be quired, however i saw something in the logs about it. to get functional KVM there's probably more configuration to apply to the container
              "kvm"
            ];
            createHome = true;
            linger = true;
          };

          environment.systemPackages = with pkgs; [
            flake.packages.${system}.hivello

            glxinfo
            xterm
            alacritty

            fluxbox
            xdg-utils

            (pkgs.writeShellScriptBin "hivello-strace" ''
              strace --follow-forks --no-abbrev --string-limit=128 --decode-fds=all --decode-pids=comm Hivello "$@" 2>&1 | tee ~/hivello.strace
            '')
          ];

          programs.nix-ld = {
            enable = true;
            libraries = with pkgs;
            # TODO: not sure if this is required
              [
                intel-vaapi-driver
                libvdpau-va-gl
                intel-media-driver
                libva-utils
              ]
              ++ flake.packages.${system}.hivello.meta.passthru.dependencies;
          };

          nix.settings.experimental-features = [
            "nix-command"
            "flakes"
          ];

          programs.turbovnc.ensureHeadlessSoftwareOpenGL = true;
          hardware.opengl = {
            enable = true;
            # TODO: not sure if this is required
            extraPackages = with pkgs; [
              mesa.drivers
              intel-vaapi-driver
              libvdpau-va-gl
              intel-media-driver
              libva.out
            ];
          };

          virtualisation.containers.containersConf.settings = {
            # these work around lack of permissions
            containers = {
              keyring = false;
              pidns = "host";
            };
          };

          virtualisation.podman = {
            enable = true;
            dockerSocket.enable = true;

            # Create a `docker` alias for podman, to use it as a drop-in replacement
            dockerCompat = true;
            # Required for containers under podman-compose to be able to talk to each other.
            # defaultNetwork.settings = {
            #   dns_enabled = false;
            # };

            # optimize later
            autoPrune.enable = false;
          };

          programs.firefox.enable = true;
          fonts = {
            enableDefaultPackages = true;
            fontconfig = {
              defaultFonts = {
                serif = [
                  "Liberation Serif"
                  "Vazirmatn"
                ];
                sansSerif = [
                  "Ubuntu"
                  "Vazirmatn"
                ];
                monospace = ["Ubuntu Mono"];
              };
            };
          };

          # this causes the systemd session to start for the user, which will in turn activate the xvnc service.
          services.getty.autologinUser = "dev";
          systemd.user.services.xvnc = {
            unitConfig.ConditionUser = "dev";

            enable = true;

            after = ["network.target"];
            wantedBy = [
              "default.target"
              "multi-user.target"
            ];

            path = config.environment.systemPackages;

            # TODO: not sure if this is required
            environment.LIBVA_DRIVER_NAME = "iHD";

            script = let
              xsession = pkgs.writeShellScript "inner" ''
                # run a terminal by default.
                alacritty &

                # TODO: fluxbox is rudimentary and we might need something
                # richer it works for testing for now.
                exec fluxbox
              '';
            in
              builtins.toString (
                pkgs.writeShellScript "xvnc" ''
                  set -xeE -o pipefail
                  ${lib.getExe' pkgs.turbovnc "Xvnc"} :0 \
                    -iglx -auth $HOME/.Xauthority \
                    -geometry 1024x768 -depth 24 \
                    -rfbwait 5000 \
                    -deferupdate 1 \
                    -securitytypes none \
                    -localhost \
                    &
                  # Xvnc takes a moment before it can be used
                  sleep 1
                  # the wrapper takes care of initialising expected variables for the graphical session
                  DISPLAY=":0" ${config.services.xserver.displayManager.sessionData.wrapper} ${xsession}
                ''
              );
          };

          services.openssh.enable = true;
          services.openssh.ports = [2200];

          # disabled in favor of the Xvnc solution
          services.openssh.settings.X11Forwarding = false;
        };
      };
    };
  }).overrideAttrs
  {
    meta.platforms = with nixpkgs.lib; lists.intersectLists platforms.linux platforms.x86_64;
  }

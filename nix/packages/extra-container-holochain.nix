/*
This file is a package that configures the container.

This can be run on a nixos machine (that has extra-containers installed ?) using:
$ nix run --extra-experimental-features "nix-command flakes" --refresh github:holo-host/holo-host#extra-container-holochain -- --restart-changed

optionally deploy locally to a dev machine:

$ nix copy --no-check-sigs "$(nix build --print-out-paths .#packages.x86_64-linux.extra-container-holochain)" --to  'ssh-ng://root@towards-allograph.dmz.internal'
*/

{
  inputs,
  system,
  flake,
  pkgs,
  nixpkgs ? inputs.nixpkgs-2411,
  index ? 0,
  adminWebsocketPort ? 8000 + index,
  containerName ? "holochain${builtins.toString index}",
  autoStart ? false,
  # these are passed to holochain
  bootstrapUrl ? null,
  signalUrl ? null,
  stunUrls ? null,
  holochainFeatures ? null,
  holochainVersion ? null,
  privateNetwork ? true,
  # hc-http-gw related args
  httpGwEnable ? false,
  httpGwAllowedAppIds ? [],
  httpGwPort ? null,  # dynamically set based on `portAllocation.HOLOCHAIN_HTTP_GW_PORT_DEFAULT` below
  # TODO: support
  # httpGwAllowedFns ? { },
}:

let
  portAllocation = import ../lib/port-allocation.nix { inherit (pkgs) lib; };

  # Get the standard port from the port allocation lib
  httpGwPortDefault = portAllocation.HOLOCHAIN_HTTP_GW_PORT_DEFAULT;
in

(pkgs.lib.makeOverridable (args: let
  
  hcAdminPort = args.adminWebsocketPort;
  httpGwPortOverride = args.httpGwPort;

  # Dynamically calculate the default httpGwPort if no override is provided
  httpGwPortDynamicDefault = if args.privateNetwork then httpGwPortDefault else (httpGwPortDefault + args.index);
  httpGwPort = if httpGwPortOverride != null then httpGwPortOverride else httpGwPortDynamicDefault;
  
  # Calculate ports for this container
  containerPorts = portAllocation.allocatePorts {
    basePorts = portAllocation.standardPorts.holochain;
    containerName = args.containerName;
    index = args.index;
    privateNetwork = args.privateNetwork;
  };
  
  httpGwPortForContainer = if args.privateNetwork then httpGwPortDefault else containerPorts.httpGateway;
  
  # Base IP offset for private network addressing to avoid conflicts with common ranges
  # This ensures container networks don't conflict with:
  # - 10.0.1.x (common router/gateway ranges)  
  # - 10.0.2.x (QEMU default network)
  # - 10.0.10.x (common VPN ranges)
  # - 10.0.20.x (common Docker networks)
  # Each container gets a unique /30 subnet: 10.0.(baseOffset + index).0/30
  privateNetworkBaseOffset = 85;
  
  package = inputs.extra-container.lib.buildContainers {
    # The system of the container host
    inherit system;

    # Optional: Set nixpkgs.
    # If unset, the nixpkgs input of extra-container flake is used
    nixpkgs = args.nixpkgs;

    # Only set this if the `system.stateVersion` of your container
    # host is < 22.05
    # legacyInstallDirs = true;

    # Set this to disable `nix run` support
    # addRunner = false;

    config = {
      # Allow container 5m startup time to allow holochain service to start properly.
      # (NB: This is needed to accomodate the network updates in holochain versions >= v0.5+)
      # Container timeout and socat port forwarding services for extra-containers
      systemd.services = pkgs.lib.mkMerge [
        # Base container timeout configuration
        {
          "container@${args.containerName}" = {
            serviceConfig = {
              TimeoutStartSec = pkgs.lib.mkForce "300s";  # 5 minutes timeout (override default 1min)
            };
          };
        }
        
        # socat-based port forwarding services optimized for extra-containers
        # These services provide reliable port forwarding when privateNetwork=true
        # by creating TCP tunnels that bypass systemd-nspawn's unreliable forwardPorts
        (pkgs.lib.mkIf args.privateNetwork {
          "socat-${args.containerName}-admin" = {
            description = "socat port tunnel for ${args.containerName} admin websocket (${builtins.toString args.adminWebsocketPort})";
            
            # Proper extra-containers lifecycle management
            after = [ 
              "container@${args.containerName}.service" 
              "network-online.target"
            ];
            wants = [ 
              "container@${args.containerName}.service"
              "network-online.target" 
            ];
            wantedBy = [ "multi-user.target" ];
            
            # Bind to container lifecycle - this ensures the socat service stops when container stops
            bindsTo = [ "container@${args.containerName}.service" ];
            
            serviceConfig = {
              Type = "exec";
              Restart = "always";
              RestartSec = "2s";
              
              # Wait for container network interface to be ready (extra-containers specific)
              ExecStartPre = [
                "${pkgs.bash}/bin/bash -c 'timeout=30; while [ $timeout -gt 0 ] && ! ${pkgs.iproute2}/bin/ip route show | grep -q \"10.0.${builtins.toString (privateNetworkBaseOffset + args.index)}.0/30\"; do sleep 1; timeout=$((timeout-1)); done'"
              ];
              
              # Create the socat tunnel from host to container
              ExecStart = "${pkgs.socat}/bin/socat TCP-LISTEN:${builtins.toString args.adminWebsocketPort},fork,reuseaddr TCP:10.0.${builtins.toString (privateNetworkBaseOffset + args.index)}.2:${builtins.toString args.adminWebsocketPort}";
              
              # Clean shutdown handling
              KillMode = "mixed";
              KillSignal = "SIGTERM";
              TimeoutStopSec = "10s";
            };
          };
        })
        
        # socat service for HTTP gateway (when enabled)
        (pkgs.lib.mkIf (args.privateNetwork && args.httpGwEnable) {
          "socat-${args.containerName}-httpgw" = {
            description = "socat port tunnel for ${args.containerName} HTTP gateway (${builtins.toString httpGwPort})";
            
            # Proper extra-containers lifecycle management
            after = [ 
              "container@${args.containerName}.service"
              "network-online.target"
            ];
            wants = [ 
              "container@${args.containerName}.service"
              "network-online.target"
            ];
            wantedBy = [ "multi-user.target" ];
            
            # Bind to container lifecycle
            bindsTo = [ "container@${args.containerName}.service" ];
            
            serviceConfig = {
              Type = "exec";
              Restart = "always";
              RestartSec = "2s";
              
              # Wait for container network to be ready
              ExecStartPre = [
                "${pkgs.bash}/bin/bash -c 'timeout=30; while [ $timeout -gt 0 ] && ! ${pkgs.iproute2}/bin/ip route show | grep -q \"10.0.${builtins.toString (privateNetworkBaseOffset + args.index)}.0/30\"; do sleep 1; timeout=$((timeout-1)); done'"
              ];
              
              # Create the socat tunnel
              ExecStart = "${pkgs.socat}/bin/socat TCP-LISTEN:${builtins.toString httpGwPort},fork,reuseaddr TCP:10.0.${builtins.toString (privateNetworkBaseOffset + args.index)}.2:${builtins.toString httpGwPortDefault}";
              
              # Clean shutdown handling
              KillMode = "mixed";
              KillSignal = "SIGTERM";
              TimeoutStopSec = "10s";
            };
          };
        })
      ];
  
        containers."${args.containerName}" = {
        privateNetwork = args.privateNetwork;
        autoStart = args.autoStart;
        
        # Network configuration for systemd-nspawn port forwarding
        # NB: These addresses are required for proper iptables rule creation
        # We're using unique IP addresses based on container index to avoid conflicts
        hostAddress = pkgs.lib.mkIf args.privateNetwork "10.0.${builtins.toString (privateNetworkBaseOffset + args.index)}.1";
        localAddress = pkgs.lib.mkIf args.privateNetwork "10.0.${builtins.toString (privateNetworkBaseOffset + args.index)}.2";
        
        # Additional network configuration for systemd-nspawn port forwarding
        # Enable virtual ethernet and proper network bridge creation
        extraFlags = pkgs.lib.optionals args.privateNetwork [
          "--network-veth"
          "--resolv-conf=bind-host"
        ];
        


        # Port forwarding for hc-http-gw when using private network
        forwardPorts = pkgs.lib.optionals (args.privateNetwork && args.httpGwEnable) [
          {
            containerPort = httpGwPortDefault;  # Standard port inside container
            hostPort = httpGwPort;  # Dynamic port on host
            protocol = "tcp";
          }
        ] ++ pkgs.lib.optionals (args.privateNetwork) [
          # Always forward hc admin websocket port for management access when private network is enabled
          {
            containerPort = hcAdminPort;
            hostPort = hcAdminPort; 
            protocol = "tcp";
          }
        ];

        # `specialArgs` is available in nixpkgs > 22.11
        # This is useful for importing flakes from modules (see nixpkgs/lib/modules.nix).
        # specialArgs = { inherit inputs; };

        bindMounts."/etc/hosts" = {
          hostPath = "/etc/hosts";
          isReadOnly = true;
        };

        config = {
          lib,
          options,
          ...
        }: {
          # in case the container shares the host network, don't mess with the firewall rules.
          networking.firewall.enable = args.privateNetwork;
          networking.useHostResolvConf = pkgs.lib.mkForce (!args.privateNetwork);
          
          # Configure systemd-networkd and systemd-resolved for proper container networking
          systemd.network = pkgs.lib.mkIf args.privateNetwork {
            enable = true;
            networks."80-container-host0" = {
              matchConfig.Virtualization = "container";
              matchConfig.Name = "host0";
              networkConfig = {
                DHCP = "yes";
                LinkLocalAddressing = "yes";
                LLDP = "yes";
                EmitLLDP = "customer-bridge";
              };
              dhcpV4Config.UseTimezone = "yes";
            };
          };
          
          # Enable systemd-resolved for container DNS when using private networking
          services.resolved = pkgs.lib.mkIf args.privateNetwork {
            enable = true;
            llmnr = "false";  # disable link-local multicast name resolution
          };

          imports = [
            flake.nixosModules.holochain
            flake.nixosModules.hc-http-gw
          ];
          
          # Add debugging tools to the container
          environment.systemPackages = with pkgs; [
            nettools  # for netstat
          ];

          holo.holochain =
            (
              {
                adminWebsocketPort = args.adminWebsocketPort;
                # NB: all holochain version handling logic is now located within the holochain nixos module.
                features = args.holochainFeatures;
            }
            )
            // (lib.optionalAttrs (args.holochainVersion != null) {version = args.holochainVersion;})
            // (lib.optionalAttrs (args.bootstrapUrl != null) {bootstrapServiceUrl = args.bootstrapUrl;})
            // (lib.optionalAttrs (args.signalUrl != null) {webrtcTransportPoolSignalUrl = args.signalUrl;})
            // (lib.optionalAttrs (args.stunUrls != null) {webrtcTransportPoolIceServers = args.stunUrls;})

            # TODO: add support for httpGwAllowedFns ?
            ;

          holo.hc-http-gw = {
            enable = args.httpGwEnable;
            adminWebsocketUrl = "ws://127.0.0.1:${builtins.toString args.adminWebsocketPort}";
            allowedAppIds = args.httpGwAllowedAppIds;
            # Use standard port inside container when `privateNetwork=true`, otherwise use dynamic port
            listenPort = httpGwPortForContainer;
            # allowedFnsPerAppId = httpGwAllowedFns;
          };
        };
      };
    };
  };

  packageWithPlatformFilter = package.overrideAttrs {
    meta.platforms = with nixpkgs.lib;
      lists.intersectLists platforms.linux (platforms.x86_64 ++ platforms.aarch64);
  };

  packageWithPlatformFilterAndTest = packageWithPlatformFilter.overrideAttrs {
    passthru.tests = {
      # Test with host networking (ie: when private networking is set to false)
      integration-host-network = pkgs.testers.runNixOSTest (
        {
          nodes,
          lib,
          ...
        }: {
          name = "host-agent-integration-nixos-host-network";
          meta.platforms = lib.lists.intersectLists lib.platforms.linux lib.platforms.x86_64;

          nodes.host = {...}: {
            imports = [
              inputs.extra-container.nixosModules.default
            ];
            
            # Add netcat for testing
            environment.systemPackages = with pkgs; [
              netcat-gnu
            ];
          };

          testScript = _: let
            # Create a test package with host networking (workaround for systemd-nspawn port forwarding issues)
            testPackage = (pkgs.callPackage (import ./extra-container-holochain.nix) {
              inherit inputs system flake pkgs nixpkgs;
              privateNetwork = false;  # Use host networking for the test
              inherit (args) 
                index adminWebsocketPort containerName autoStart bootstrapUrl 
                signalUrl stunUrls holochainFeatures holochainVersion 
                httpGwEnable httpGwAllowedAppIds httpGwPort;
            });
          in ''
            host.start()
            host.wait_for_unit("multi-user.target")
            host.succeed("extra-container create ${testPackage}")

            # Ensure the port is closed before starting the holochain container
            host.wait_for_closed_port(${builtins.toString args.adminWebsocketPort}, timeout = 1)

            host.succeed("extra-container start ${args.containerName}")
            
            # Use `Type="notify"`to ensure systemd waits for holochain to signal readiness
            # NB: This means when the service is active, the admin websocket should be ready
            host.wait_until_succeeds("systemctl -M ${args.containerName} is-active holochain", timeout = 60)
            
            # Make the port should be directly accessible on the host for test
            host.wait_for_open_port(${builtins.toString args.adminWebsocketPort}, timeout = 10)
            
            # Verify that the port is responding to connections
            host.succeed("nc -z localhost ${builtins.toString args.adminWebsocketPort}")
            
            # Test state persistence by stopping and restarting the container
            host.succeed("extra-container stop ${args.containerName}")
            host.wait_for_closed_port(${builtins.toString args.adminWebsocketPort}, timeout = 10)
            
            # Restart the container and verify state is preserved
            host.succeed("extra-container start ${args.containerName}")
            host.wait_until_succeeds("systemctl -M ${args.containerName} is-active holochain", timeout = 60)
            host.wait_for_open_port(${builtins.toString args.adminWebsocketPort}, timeout = 10)
            host.succeed("nc -z localhost ${builtins.toString args.adminWebsocketPort}")
            
            # Verify that holochain state directory exists and persists data
            host.succeed("test -d /var/lib/nixos-containers/${args.containerName}/var/lib/holochain")
          '';
        }
      );

      # Test with private networking and port forwarding (currently failing due to systemd-nspawn compatibility)
      integration-private-network = pkgs.testers.runNixOSTest (
        {
          nodes,
          lib,
          ...
        }: {
          name = "host-agent-integration-nixos-private-network";
          meta.platforms = lib.lists.intersectLists lib.platforms.linux lib.platforms.x86_64;

          nodes.host = {...}: {
            imports = [
              inputs.extra-container.nixosModules.default
            ];
            
            # Add netcat for testing
            environment.systemPackages = with pkgs; [
              netcat-gnu
            ];
          };

          testScript = _: let
            # Create a test package with private networking and port forwarding
            testPackage = (pkgs.callPackage (import ./extra-container-holochain.nix) {
              inherit inputs system flake pkgs nixpkgs;
              privateNetwork = true;  # Use private networking with port forwarding
              inherit (args) 
                index adminWebsocketPort containerName autoStart bootstrapUrl 
                signalUrl stunUrls holochainFeatures holochainVersion 
                httpGwEnable httpGwAllowedAppIds httpGwPort;
            });
          in ''
            host.start()
            host.wait_for_unit("multi-user.target")
            host.succeed("extra-container create ${testPackage}")

            # Ensure the port is closed before starting the holochain container
            host.wait_for_closed_port(${builtins.toString args.adminWebsocketPort}, timeout = 1)

            host.succeed("extra-container start ${args.containerName}")
            
            # Use `Type="notify"`to ensure systemd waits for holochain to signal readiness
            # NB: This means when the service is active, the admin websocket should be ready
            host.wait_until_succeeds("systemctl -M ${args.containerName} is-active holochain", timeout = 60)
            
            # Test port forwarding via socat
            # NB: The socat service should create a reliable tunnel from host to container
            host.wait_for_open_port(${builtins.toString args.adminWebsocketPort}, timeout = 30)
            host.succeed("nc -z localhost ${builtins.toString args.adminWebsocketPort}")
            
            # Verify that holochain state directory exists inside the container
            host.succeed("machinectl shell ${args.containerName} /usr/bin/env test -d /var/lib/holochain")
            
            # Test state persistence by stopping and restarting the container
            host.succeed("extra-container stop ${args.containerName}")
            
            # Restart the container and verify state is preserved
            host.succeed("extra-container start ${args.containerName}")
            host.wait_until_succeeds("systemctl -M ${args.containerName} is-active holochain", timeout = 60)
            
            # Verify that holochain state directory still exists and persists data after restart
            host.succeed("machinectl shell ${args.containerName} /usr/bin/env test -d /var/lib/holochain")
            host.succeed("test -d /var/lib/nixos-containers/${args.containerName}/var/lib/holochain")
          '';
        }
      );
    };
  };
in
  packageWithPlatformFilterAndTest
)) {
  inherit
    inputs
    system
    flake
    pkgs
    nixpkgs
    privateNetwork
    index
    adminWebsocketPort
    containerName
    autoStart
    bootstrapUrl
    signalUrl
    stunUrls
    holochainFeatures
    holochainVersion
    httpGwEnable
    httpGwAllowedAppIds
    httpGwPort
    ;
}

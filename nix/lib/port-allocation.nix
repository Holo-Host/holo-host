# Port allocation utils for container services
# NB: The purpose of this lib is to create a shared strategy across all nixos modules to avoid port collisions when running multiple containers

{ lib }:

{
  HOLOCHAIN_HTTP_GW_PORT_DEFAULT = 8090;

  # Allocate ports for a container with the given base ports and index
  allocatePorts = { basePorts, containerName ? "", index ? 0, privateNetwork ? false }:
    let
      # Generate a deterministic port offset based on container name/index
      generatePortOffset = containerName: index: 
        let
          # Hash the container name to create deterministic offset
          nameHash = builtins.hashString "sha256" containerName;
          # Use the container name and hash len to generate a simple offset
          hashLength = lib.stringLength nameHash;
          nameLength = lib.stringLength containerName;
          baseOffset = lib.mod (hashLength + nameLength) 50;
        in
          baseOffset + (index * 10);
      
      offset = if privateNetwork then 0 else (generatePortOffset containerName index);
    in
      lib.mapAttrs (name: basePort: basePort + offset) basePorts;

  standardPorts = {
    holochain = {
      adminWebsocket = 8000;
      httpGateway = 8090;  # Same as HOLOCHAIN_HTTP_GW_PORT_DEFAULT
    };
    nats = {
      client = 4222;
      websocket = 443;
      leafnode = 7422;
    };
  };

  # Generate port forwarding rules for containers with a private network
  generatePortForwarding = { containerPorts, hostPorts }:
    lib.mapAttrsToList (name: containerPort: {
      inherit containerPort;
      hostPort = hostPorts.${name};
      protocol = "tcp";
    }) containerPorts;
} 
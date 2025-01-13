{
  flake,
  pkgs,
  ...
}:

pkgs.testers.runNixOSTest (
  { nodes, lib, ... }:
  {
    name = "host-agent-integration-nixos";
    meta.platforms = lib.lists.intersectLists lib.platforms.linux lib.platforms.x86_64;

    # TODO: add a NATS server which is used to test the leaf connection
    # nodes.hub = { };

    nodes.agent =
      { config, ... }:
      {
        imports = [
          flake.nixosModules.holo-nats-server
          flake.nixosModules.holo-agent
        ];

        holo.nats-server.enable = true;

        holo.agent = {
          enable = true;
          autoStart = false;
          rust = {
            log = "trace";
            backtrace = "trace";
          };

          nats = {
            url = "127.0.0.1:${builtins.toString config.services.nats.port}";
            hubServerUrl = "127.0.0.1:${builtins.toString config.services.nats.settings.leafnodes.port}";
          };
        };
      };

    # takes args which are currently removed by deadnix:
    # { nodes, ... }
    testScript = _: ''
      agent.start()

      agent.wait_for_unit("nats.service")

      # TODO: fix after/require settings of the holo-agent service to make autoStart work
      agent.succeed("systemctl start holo-agent")
      agent.wait_for_unit("holo-agent")
    '';
  }
)

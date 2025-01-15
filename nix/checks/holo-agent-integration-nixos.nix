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

    nodes.hub =
      {
        ...

      }:
      {
        imports = [
          flake.nixosModules.holo-nats-server
          # flake.nixosModules.holo-orchestrator
        ];

        # holo.orchestrator.enable = true;
        holo.nats-server.enable = true;
      };

    nodes.host =
      { ... }:
      {
        imports = [
          flake.nixosModules.holo-host-agent
        ];

        holo.host-agent = {
          enable = true;
          autoStart = false;
          rust = {
            log = "trace";
            backtrace = "trace";
          };

          nats = {
            # url = "agent:${builtins.toString config.services.nats.port}";
            hubServerUrl = "hub:${builtins.toString nodes.hub.holo.nats-server.leafnodePort}";
          };
        };
      };

    # takes args which are currently removed by deadnix:
    # { nodes, ... }
    testScript =
      _:
      let
        natsCli = lib.getExe pkgs.natscli;
        hubTestScript = pkgs.writeShellScript "cmd" ''
          ${natsCli} pub -s 'nats://127.0.0.1:${builtins.toString nodes.hub.holo.nats-server.port}' --count=10 WORKLOAD.start '{"message":"hello"}'
        '';

        hostTestScript = pkgs.writeShellScript "cmd" ''
          ${natsCli} sub -s 'nats://127.0.0.1:${builtins.toString nodes.host.holo.host-agent.nats.listenPort}' --count=10 'WORKLOAD.>'
        '';
      in
      ''
        hub.start()
        hub.wait_for_unit("nats.service")
        hub.succeed("${hubTestScript}")

        host.start()
        # agent.wait_for_unit("nats.service")

        # TODO: fix after/require settings of the host-agent service to make autoStart work
        host.succeed("systemctl start holo-host-agent")
        host.wait_for_unit("holo-host-agent")

        host.succeed("${hostTestScript}")
      '';
  }
)

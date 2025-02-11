{
  flake,
  pkgs,
  ...
}:

pkgs.testers.runNixOSTest (
  { nodes, lib, ... }:
  let
    hubIP = (pkgs.lib.head nodes.hub.networking.interfaces.eth1.ipv4.addresses).address;
    hubJsDomain = "hub";

    hostUseOsNats = false;
  in
  {
    name = "auth-integration-nixos";
    meta.platforms = lib.lists.intersectLists lib.platforms.linux lib.platforms.x86_64;

    defaults.networking.hosts = {
      hubIP = [ "${nodes.hub.networking.fqdn}" ];
    };

    nodes.hub =
      { ... }:
      {
        imports = [
          flake.nixosModules.holo-nats-server
          flake.nixosModules.holo-orchestrator
        ];

        networking.domain = "local";

        holo.nats-server.enable = true;
        holo.orchestrator.enable = true;
        services.nats.settings = {
          system_account = "SYS";
          include main-resolver.conf

          jetstream = {
            domain = "${hubJsDomain}";
            enabled = true;
          };

          # logging options
          debug = true;
          trace = false;
          logtime = false;
        };
      };

    nodes.host =
      { ... }:
      {
        imports = [
          flake.nixosModules.holo-nats-server
          flake.nixosModules.holo-host-agent
        ];

        holo.host-agent = {
          enable = !hostUseOsNats;
          rust = {
            log = "trace";
            backtrace = "trace";
          };

          nats.hub.url = "wss://${nodes.hub.networking.fqdn}:${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}";
          nats.hub.tlsInsecure = true;
        };

        holo.nats-server.enable = hostUseOsNats;
        services.nats.settings = {
          jetstream = {
            domain = "leaf";
            enabled = true;
          };

          leafnodes = {
            remotes = [
              { url = "nats://${hubIP}:${builtins.toString nodes.hub.holo.nats-server.leafnodePort}"; }
            ];
          };

          # logging options
          debug = true;
          trace = false;
          logtime = false;
        };
      };

    # takes args which are currently removed by deadnix:
    # { nodes, ... }
    testScript =
      _:
      let
        natsCli = lib.getExe pkgs.natscli;
        nsc = lib.getExe pkgs.nsc;
        hub
        hubAuthTestScript =
          let
            natsServer = "nats://127.0.0.1:${builtins.toString nodes.hub.holo.nats-server.port}";
          in
          pkgs.writeShellScript "cmd" ''
            set -xe

            ${nsc} describe account AUTH --field authcallout | jq -r
            # test that AUTH account has `auth_callout` in its jwt json

            ${natsCli} context save SYS_USER --nsc "nsc://HOLO/SYS/sys.creds"
            ${natsCli} -s "${natsServer}" stream ls --context SYS_USER
            # test that 1 stream exists
            ${natsCli} -s "${natsServer}" stream info --json ${WorkloadStreamName} --context SYS_USER
            # test that WORKLOAD stream *is* the single stream

            ${natsCli} -s "${natsServer}" micro ls --context SYS_USER
            # test that 1 service exists
            ${natsCli} -s "${natsServer}" micro info --json ${AuthStreamName} --context SYS_USER
            # test that AUTH service *is* the single service registered
          '';

        hostAuthTestScript =
          let
            natsServer = "nats://127.0.0.1:${builtins.toString nodes.host.holo.host-agent.nats.listenPort}";
          in
          pkgs.writeShellScript "cmd" ''
            set -xe

            ${natsCli} context save HOST_USER --nsc "nsc://HOLO/WORKLOAD/host.creds"
            ${natsCli} context save SYS_USER --nsc "nsc://HOLO/SYS/sys.creds"

            ${natsCli} -s "${natsServer}" stream ls --context SYS_USER
            ${natsCli} -s "${natsServer}" stream info --json ${WorkloadStreamName} --context SYS_USER

            ${natsCli} -s '${natsServer}' sub --stream "${WorkloadStreamName}" '${WorkloadStreamName}.>' --count=10 --context SYS_USER
            ${natsCli} -s '${natsServer}' pub "${WorkloadStreamName}.hello" '{"message":"hello"}' --js-domain ${hubJsDomain} --count=10 --context HOST_USER
          '';
      in
      ''
        with subtest("start the hub and run the hub auth test"):
          hub.start()
          hub.wait_for_unit("nats.service")
          hub.wait_for_open_port(port = ${builtins.toString nodes.hub.holo.nats-server.websocket.port}, timeout = 1)

          hub.wait_for_unit("caddy.service")
          hub.wait_for_open_port(port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 1)

          hub.succeed("${hubAuthTestScript}")

        with subtest("start the host and run the host auth test"):
          host.start()
          host.wait_for_unit('holo-host-agent')
          host.succeed("${hostAuthTestScript}", timeout = 10)

        with subtest("verify that holo-host-agent spins up leaf server and wait for it to be ready"):
          host.wait_for_unit('nats.service')
          host.wait_for_open_port(addr = "${nodes.hub.networking.fqdn}", port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 10)
      '';
  }
)

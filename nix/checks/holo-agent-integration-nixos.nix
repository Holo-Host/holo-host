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
  in
  {
    name = "host-agent-integration-nixos";
    meta.platforms = lib.lists.intersectLists lib.platforms.linux lib.platforms.x86_64;

    defaults.networking.hosts = {
      "${hubIP}" = [
        "${nodes.hub.networking.fqdn}"
      ];
    };

    nodes.hub =
      { ... }:
      {
        imports = [
          flake.nixosModules.holo-nats-server
          # flake.nixosModules.holo-orchestrator
        ];

        networking.domain = "local";

        # holo.orchestrator.enable = true;
        holo.nats-server.enable = true;
        services.nats.settings = {
          accounts = {
            SYS = {
              users = [
                {
                  user = "admin";
                  "password" = "admin";
                }
              ];
            };
          };
          system_account = "SYS";

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

    nodes.host1 =
      { ... }:
      {
        imports = [
          flake.nixosModules.holo-host-agent
        ];

        holo.host-agent = {
          enable = true;
          rust = {
            log = "trace";
            backtrace = "trace";
          };

          nats.hub.url = "wss://${nodes.hub.networking.fqdn}:${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}";
          nats.hub.tlsInsecure = true;
        };
      };

    # takes args which are currently removed by deadnix:
    # { nodes, ... }
    testScript =
      _:
      let
        natsCli = lib.getExe pkgs.natscli;
        testStreamName = "INTEGRATION";

        _testStreamHubConfig = builtins.toFile "stream.conf" ''
          {
              "name": "${testStreamName}",
              "subjects": [
                  "${testStreamName}",
                  "${testStreamName}.\u003e"
              ],
              "retention": "limits",
              "max_consumers": -1,
              "max_msgs_per_subject": -1,
              "max_msgs": -1,
              "max_bytes": -1,
              "max_age": 0,
              "max_msg_size": -1,
              "storage": "memory",
              "discard": "old",
              "num_replicas": 1,
              "duplicate_window": 120000000000,
              "sealed": false,
              "deny_delete": false,
              "deny_purge": false,
              "allow_rollup_hdrs": false,
              "allow_direct": true,
              "mirror_direct": false,
              "consumer_limits": {}
          }
        '';
        _testStreamLeafConfig = builtins.toFile "stream.conf" ''
          {
              "name": "${testStreamName}",
              "retention": "limits",
              "max_consumers": -1,
              "max_msgs_per_subject": -1,
              "max_msgs": -1,
              "max_bytes": -1,
              "max_age": 0,
              "max_msg_size": -1,
              "storage": "memory",
              "discard": "old",
              "num_replicas": 1,
              "mirror": {
                  "name": "${testStreamName}",
                  "external": {
                      "api": "$JS.${hubJsDomain}.API",
                      "deliver": ""
                  }
              },
              "sealed": false,
              "deny_delete": false,
              "deny_purge": false,
              "allow_rollup_hdrs": false,
              "allow_direct": true,
              "mirror_direct": false,
              "consumer_limits": {}
          }
        '';
        hubTestScript =
          let
            natsServer = "nats://127.0.0.1:${builtins.toString nodes.hub.holo.nats-server.port}";
          in
          pkgs.writeShellScript "cmd" ''
            set -xe

            ${natsCli} -s "${natsServer}" stream add ${testStreamName} --config ${_testStreamHubConfig}
            ${natsCli} -s "${natsServer}" pub --count=10 "${testStreamName}.integrate" --js-domain ${hubJsDomain} '{"message":"hello"}'
            ${natsCli} -s "${natsServer}" stream ls
            ${natsCli} -s "${natsServer}" sub --stream "${testStreamName}" "${testStreamName}.>" --count=10
          '';

        hostTestScript =
          let
            natsServer = "nats://127.0.0.1:${builtins.toString nodes.host1.holo.host-agent.nats.listenPort}";
          in
          pkgs.writeShellScript "cmd" ''
            set -xe

            ${natsCli} -s "${natsServer}" stream add ${testStreamName} --config ${_testStreamLeafConfig}
            ${natsCli} -s "${natsServer}" stream ls
            ${natsCli} -s "${natsServer}" stream info --json ${testStreamName}
            ${natsCli} -s '${natsServer}' sub --stream "${testStreamName}" '${testStreamName}.>' --count=10
          '';
      in
      ''
        with subtest("start the hub and run the testscript"):
          hub.start()
          hub.wait_for_unit("nats.service")
          hub.wait_for_open_port(port = ${builtins.toString nodes.hub.holo.nats-server.websocket.port}, timeout = 1)

          hub.wait_for_unit("caddy.service")
          hub.wait_for_open_port(port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 1)

          hub.succeed("${hubTestScript}")

        with subtest("starting the host1 and waiting for holo-host-agent to be ready"):
          host1.start()
          host1.wait_for_unit('holo-host-agent')

          host1.wait_for_open_port(addr = "${nodes.hub.networking.fqdn}", port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 10)

        with subtest("running the host1 testscript"):
          host1.succeed("${hostTestScript}", timeout = 10)
      '';
  }
)

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

    mkHost =
      _:
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

    nodes.host1 = mkHost { };
    nodes.host2 = mkHost { };
    nodes.host3 = mkHost { };

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

        natsCmdHub = "${natsCli} -s nats://127.0.0.1:${builtins.toString nodes.hub.holo.nats-server.port}";
        natsCmdHosts = "${natsCli} -s nats://127.0.0.1:${builtins.toString nodes.host1.holo.host-agent.nats.listenPort}";

        hubTestScript = pkgs.writeShellScript "cmd" ''
          set -xe
          ${natsCmdHub} stream add ${testStreamName} --config ${_testStreamHubConfig}
          ${natsCmdHub} pub --count=10 "${testStreamName}.integrate" --js-domain ${hubJsDomain} '{"message":"hello"}'
          ${natsCmdHub} stream ls
          ${natsCmdHub} sub --stream "${testStreamName}" "${testStreamName}.>" --count=10
        '';

        hostTestScript = pkgs.writeShellScript "cmd" ''
          set -xe

          ${natsCmdHosts} stream add ${testStreamName} --config ${_testStreamLeafConfig}
          ${natsCmdHosts} stream ls
          ${natsCmdHosts} stream info --json ${testStreamName}
          ${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.>' --count=10
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

        with subtest("start the hosts and ensure they have TCP level connectivity to the hub"):
          host1.start()
          host2.start()
          host3.start()

          host1.wait_for_unit('holo-host-agent')
          host2.wait_for_unit('holo-host-agent')
          host3.wait_for_unit('holo-host-agent')

          host1.wait_for_open_port(addr = "${nodes.hub.networking.fqdn}", port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 10)
          host2.wait_for_open_port(addr = "${nodes.hub.networking.fqdn}", port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 10)
          host3.wait_for_open_port(addr = "${nodes.hub.networking.fqdn}", port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 10)

        with subtest("running the testscript on the hosts"):
          host1.succeed("${hostTestScript}", timeout = 10)
          host2.succeed("${hostTestScript}", timeout = 10)
          host3.succeed("${hostTestScript}", timeout = 10)

        with subtest("publish more messages from the hub and ensure they arrive on all hosts"):
          hub.succeed("${pkgs.writeShellScript "script" ''
            ${natsCmdHub} pub --count=10 "${testStreamName}.host1" --js-domain ${hubJsDomain} '{"message":"hello host1"}'
            ${natsCmdHub} pub --count=10 "${testStreamName}.host2" --js-domain ${hubJsDomain} '{"message":"hello host2"}'
            ${natsCmdHub} pub --count=10 "${testStreamName}.host3" --js-domain ${hubJsDomain} '{"message":"hello host3"}'
          ''}", timeout = 10)

          host1.succeed("${pkgs.writeShellScript "script" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host1' --count=10''}", timeout = 10)
          host2.succeed("${pkgs.writeShellScript "script" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host2' --count=10''}", timeout = 10)
          host3.succeed("${pkgs.writeShellScript "script" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host2' --count=10''}", timeout = 10)
      '';
  }
)

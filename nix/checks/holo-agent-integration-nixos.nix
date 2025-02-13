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
          nats.store_dir = "/var/lib/holo-host-agent/store_dir";
        };
      };
  in
  {
    name = "host-agent-integration-nixos";
    meta.platforms = lib.lists.intersectLists lib.platforms.linux lib.platforms.x86_64;

    defaults.networking.hosts = {
      "${hubIP}" = [ "${nodes.hub.networking.fqdn}" ];
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
    nodes.host4 = mkHost { };
    nodes.host5 = mkHost { };

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
              "storage": "file",
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
              "storage": "file",
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

        hubTestScript = pkgs.writeShellScript "setup-hub" ''
          set -xe
          ${natsCmdHub} stream add ${testStreamName} --config ${_testStreamHubConfig}
          ${natsCmdHub} pub --count=10 "${testStreamName}.integrate" --js-domain '${hubJsDomain}' '{"message":"hello"}'
          ${natsCmdHub} stream ls
          ${natsCmdHub} sub --stream "${testStreamName}" "${testStreamName}.>" --count=10
        '';

        hostSetupScript = pkgs.writeShellScript "setup-host" ''
          set -xe

          ${natsCmdHosts} stream add ${testStreamName} --config ${_testStreamLeafConfig}
          ${natsCmdHosts} stream ls
          ${natsCmdHosts} stream info --json ${testStreamName}
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
          host4.start()
          host5.start()

          host1.wait_for_open_port(addr = "${nodes.hub.networking.fqdn}", port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 10)

          host1.wait_for_unit('holo-host-agent')
          host2.wait_for_unit('holo-host-agent')
          host3.wait_for_unit('holo-host-agent')
          host4.wait_for_unit('holo-host-agent')
          host5.wait_for_unit('holo-host-agent')

        with subtest("running the setup script on the hosts"):
          host1.succeed("${hostSetupScript}", timeout = 1)
          host2.succeed("${hostSetupScript}", timeout = 1)
          host3.succeed("${hostSetupScript}", timeout = 1)
          host4.succeed("${hostSetupScript}", timeout = 1)
          host5.succeed("${hostSetupScript}", timeout = 1)

        with subtest("wait until all hosts receive all published messages"):
          host1.succeed("${pkgs.writeShellScript "receive-all-msgs" ''set -x; ${natsCmdHosts} --trace sub --stream "${testStreamName}" '${testStreamName}.integrate' --count=10''}", timeout = 5)
          host2.succeed("${pkgs.writeShellScript "receive-all-msgs" ''set -x; ${natsCmdHosts} --trace sub --stream "${testStreamName}" '${testStreamName}.integrate' --count=10''}", timeout = 5)
          host3.succeed("${pkgs.writeShellScript "receive-all-msgs" ''set -x; ${natsCmdHosts} --trace sub --stream "${testStreamName}" '${testStreamName}.integrate' --count=10''}", timeout = 5)
          host4.succeed("${pkgs.writeShellScript "receive-all-msgs" ''set -x; ${natsCmdHosts} --trace sub --stream "${testStreamName}" '${testStreamName}.integrate' --count=10''}", timeout = 5)
          host5.succeed("${pkgs.writeShellScript "receive-all-msgs" ''set -x; ${natsCmdHosts} --trace sub --stream "${testStreamName}" '${testStreamName}.integrate' --count=10''}", timeout = 5)

        with subtest("publish more messages from the hub and ensure they arrive on all hosts"):
          hub.succeed("${pkgs.writeShellScript "script" ''
            set -xeE
            for i in `seq 1 5`; do
              ${natsCmdHub} pub --count=10 "${testStreamName}.host''${i}" --js-domain ${hubJsDomain} "{\"message\":\"hello host''${i}\"}"
            done
          ''}", timeout = 1)

          host1.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host1' --count=10''}", timeout = 5)
          host2.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host2' --count=10''}", timeout = 5)
          host3.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host3' --count=10''}", timeout = 5)
          host4.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host4' --count=10''}", timeout = 5)
          host5.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host5' --count=10''}", timeout = 5)

        with subtest("bring a host down, publish messages, bring it back up, make sure it receives all messages"):
          host5.shutdown()

          hub.succeed("${pkgs.writeShellScript "script" ''
            set -xeE
            for i in `seq 1 5`; do
              ${natsCmdHub} pub --count=10 "${testStreamName}.host''${i}" --js-domain ${hubJsDomain} "{\"message\":\"hello host''${i}\"}"
            done
          ''}", timeout = 2)

          host1.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host1' --count=10''}", timeout = 5)
          host2.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host2' --count=10''}", timeout = 5)
          host3.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host3' --count=10''}", timeout = 5)
          host4.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host4' --count=10''}", timeout = 5)

          host5.start()
          host5.wait_for_unit('holo-host-agent')
          host5.wait_until_succeeds("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host5' --count=10''}", timeout = 5)
      '';
  }
)

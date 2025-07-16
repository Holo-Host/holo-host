{
  flake,
  pkgs,
  system,
  ...
}:

pkgs.testers.runNixOSTest (
  {
    nodes,
    lib,
    ...
  }:
  let
    hubIP = (pkgs.lib.head nodes.hub.networking.interfaces.eth1.ipv4.addresses).address;
    hubJsDomain = "hub";
    hubNatsUrl = "wss://${nodes.hub.networking.fqdn}:${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}";

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

          nats.hub.url = hubNatsUrl;
          nats.hub.tlsInsecure = true;
          nats.store_dir = "/var/lib/holo-host-agent/store_dir";
        };
      };

    hostAgentCli = lib.getExe flake.packages.${system}.rust-workspace.individual.host_agent;

    # Mock workload variables
    workloadId = "test-workload-123";
    containerName = "test123";
    testDeveloper = "test-developer";
    testManifest = {
        type = "HolochainDhtV1";
        happ_binary_url = "https://gist.github.com/steveej/5443d6d15395aa23081f1ee04712b2b3/raw/c82daf7f03ef459fa9ec4f28c8eeb9602596cc22/humm-earth-core-happ.happ";
        network_seed = null;
        memproof = null;
        bootstrap_server_url = "https://bootstrap.holo.host/";
        signal_server_url = "wss://sbd.holo.host/";
        stun_server_urls = null;
        holochain_feature_flags = ["unstable-functions" "unstable-sharding" "chc" "unstable-countersigning"];
        holochain_version = "0.4.2";
        http_gw_enable = true;
        http_gw_allowed_fns = null;
      };
      defaultSystemSpecs = {
        capacity = {
          drive = 1;
          cores = 1;
        };
        avg_network_speed = 0;
        avg_uptime = 0.0;
      };

    # Mock workload deployment message
    workloadDeployMsg = builtins.toJSON {
      _id = workloadId;
      version = "0.0.1";
      metadata = {
        is_deleted = false;
        deleted_at = null;
        updated_at = "2025-06-25T14:32:35.541+00:00";
        created_at = "2025-06-25T14:32:35.541+00:00";
      };
      assigned_developer = testDeveloper;
      min_hosts = 1;
      assigned_hosts = [];
      status = {
        desired = "Running";
        actual = "Assigned";
        payload = null;
      };
      manifest = testManifest;
      system_specs = defaultSystemSpecs;
    };

    # Mock workload update message
    workloadUpdateMsg = builtins.toJSON {
      _id = workloadId;
      version = "0.0.1";
      metadata = {
        is_deleted = false;
        deleted_at = null;
        updated_at = "2025-06-25T16:20:00.000+00:00";
        created_at = "2025-06-25T14:32:35.541+00:00";
      };
      assigned_developer = testDeveloper;
      min_hosts = 1;
      assigned_hosts = [];
      status = {
        desired = "Running";
        actual = "Running";
        payload = null;
      };
      manifest = testManifest;
      system_specs = defaultSystemSpecs;
    };

    # Mock workload deletion message
    workloadDeleteMsg = builtins.toJSON {
      _id = workloadId;
      version = "0.0.1";
      metadata = {
        is_deleted = true;
        deleted_at = "2025-06-25T16:21:00.000+00:00";
        updated_at = "2025-06-25T16:21:00.000+00:00";
        created_at = "2025-06-25T14:32:35.541+00:00";
      };
      assigned_developer = testDeveloper;
      min_hosts = 1;
      assigned_hosts = [];
      status = {
        desired = "Deleted";
        actual = "Running";
        payload = null;
      };
      manifest = testManifest;
      system_specs = defaultSystemSpecs;
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
        holo.nats-server = {
          enable = true;
          websocket = {
            enable = true;
            port = 443;
            externalPort = 443;
          };
          nsc.localCredsPath = "/var/lib/nats/nsc/local";
        };
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

        natsCmdHub = "${natsCli} -s nats://127.0.0.1:${builtins.toString nodes.hub.holo.nats-server.server.port}";
        natsLocalhostUrl = "nats://127.0.0.1:${builtins.toString nodes.host1.holo.host-agent.nats.listenPort}";
        natsCmdHosts = "${natsCli} -s ${natsLocalhostUrl}";

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

        hostAgentCli = lib.getExe flake.packages.${system}.rust-workspace.individual.host_agent;

      in
      ''
        with subtest("start the hub and run the testscript"):
          hub.start()
          hub.wait_for_unit("nats.service")
          hub.wait_for_open_port(port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 1)

          hub.wait_for_unit("caddy.service")
          hub.wait_for_open_port(port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 1)

          hub.succeed("${hubTestScript}")

        with subtest("start the hosts and ensure they have TCP level connectivity to the hub"):
          host1.start()

          host1.wait_for_open_port(addr = "${nodes.hub.networking.fqdn}", port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 10)

          host1.wait_for_unit('holo-host-agent')

          with subtest("NATS connectivity to localhost and the hub using `host_agent remote ping`"):
            host1.wait_until_succeeds("${pkgs.writeShellScript "host-agent-remote-ping" ''
              set -xeE
              ${hostAgentCli} remote \
                --nats-url ${natsLocalhostUrl} \
                ping
              ${hostAgentCli} remote \
                --nats-skip-tls-verification-danger \
                --nats-url ${hubNatsUrl} \
                ping
            ''}", timeout = 10)

          host2.start()
          host3.start()
          host4.start()
          host5.start()
          host2.wait_for_unit('holo-host-agent')
          host3.wait_for_unit('holo-host-agent')
          host4.wait_for_unit('holo-host-agent')
          host5.wait_for_unit('holo-host-agent')

        with subtest("running the setup script on the hosts"):
          # Wait for NATS servers to be ready on all hosts before running setup scripts
          host1.wait_for_open_port(port = ${builtins.toString nodes.host1.holo.host-agent.nats.listenPort}, timeout = 10)
          host2.wait_for_open_port(port = ${builtins.toString nodes.host2.holo.host-agent.nats.listenPort}, timeout = 10)
          host3.wait_for_open_port(port = ${builtins.toString nodes.host3.holo.host-agent.nats.listenPort}, timeout = 10)
          host4.wait_for_open_port(port = ${builtins.toString nodes.host4.holo.host-agent.nats.listenPort}, timeout = 10)
          host5.wait_for_open_port(port = ${builtins.toString nodes.host5.holo.host-agent.nats.listenPort}, timeout = 10)
          
          host1.succeed("${hostSetupScript}", timeout = 10)
          host2.succeed("${hostSetupScript}", timeout = 10)
          host3.succeed("${hostSetupScript}", timeout = 10)
          host4.succeed("${hostSetupScript}", timeout = 10)
          host5.succeed("${hostSetupScript}", timeout = 10)

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
          ''}", timeout = 10)

          host1.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host1' --count=10''}", timeout = 5)
          host2.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host2' --count=10''}", timeout = 5)
          host3.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host3' --count=10''}", timeout = 5)
          host4.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host4' --count=10''}", timeout = 5)
          host5.succeed("${pkgs.writeShellScript "receive-specific-msgs" ''${natsCmdHosts} sub --stream "${testStreamName}" '${testStreamName}.host5' --count=10''}", timeout = 5)

        # #TODO: Add test suite for the workload host<>orchestrator deployment workflow
        # with subtest("test workload deployment and container management"):
        #   # Publish workload deployment message
        #   hub.succeed("${pkgs.writeShellScript "deploy-workload" ''
        #     set -xeE
        #     ${natsCmdHub} pub "WORKLOAD.${workloadId}.update" --js-domain ${hubJsDomain} '${workloadDeployMsg}'
        #   ''}", timeout = 1)
          
        #   # Wait for host to process the workload
        #   host1.wait_until_succeeds("${pkgs.writeShellScript "check-workload-status" ''
        #     set -xeE
        #     # Check if container was created
        #     machinectl list | grep -q "${containerName}" || exit 1
        #     # Check if container is running
        #     machinectl show "${containerName}" | grep -q "State=running" || exit 1
        #     # Check if holochain service is active
        #     systemctl -M "${containerName}" is-active holochain || exit 1
        #     # Check if admin websocket port is accessible
        #     nc -z localhost 8000 || exit 1
        #   ''}", timeout = 120)
          
        #   # Test workload update (simulates config change in db)
        #   hub.succeed("${pkgs.writeShellScript "update-workload" ''
        #     set -xeE
        #     ${natsCmdHub} pub "WORKLOAD.${workloadId}.update" --js-domain ${hubJsDomain} '${workloadUpdateMsg}'
        #   ''}", timeout = 1)
          
        #   # Wait for host to process the update
        #   host1.wait_until_succeeds("${pkgs.writeShellScript "check-updated-workload" ''
        #     set -xeE
        #     # Container should still be running after update
        #     machinectl list | grep -q "${containerName}" || exit 1
        #     machinectl show "${containerName}" | grep -q "State=running" || exit 1
        #     systemctl -M "${containerName}" is-active holochain || exit 1
        #     nc -z localhost 8000 || exit 1
        #   ''}", timeout = 60)
          
        #   # Test workload deletion
        #   hub.succeed("${pkgs.writeShellScript "delete-workload" ''
        #     set -xeE
        #     ${natsCmdHub} pub "WORKLOAD.${workloadId}.update" --js-domain ${hubJsDomain} '${workloadDeleteMsg}'
        #   ''}", timeout = 1)
          
        #   # Wait for host to stop and remove the container
        #   host1.wait_until_succeeds("${pkgs.writeShellScript "check-workload-deleted" ''
        #     set -xeE
        #     # Container should be deleted/removed
        #     ! machinectl list | grep -q "${containerName}" || exit 1
        #     # Port should be closed
        #     ! nc -z localhost 8000 || exit 1
        #   ''}", timeout = 60)

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

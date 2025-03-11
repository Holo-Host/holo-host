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

    sharedFiles = pkgs.runCommand "my-example" {nativeBuildInputs =
            [
              pkgs.coreutils
              
              ## NATS/mongodb integration tests
              pkgs.nats-server
              pkgs.nsc
            ]} ''
  echo My example command is running

  mkdir $out

  echo I can write data to the Nix store > $out/message

  echo I can also run basic commands like:

  echo ls
  ls

  echo whoami
  whoami

  echo date
  date
'';
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
          include = "main-resolver.conf";

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
        workloadStreamName = "WORKLOAD";
        authServiceName = "AUTH";

        hubAuthTestScript =
          let
            natsServer = "nats://127.0.0.1:${builtins.toString nodes.hub.holo.nats-server.port}";
          in
          pkgs.writeShellScript "cmd" ''
            set -xe

            ls "${sharedFiles}"

            AUTH_ACCOUNT_AUTH_SETTINGS="$(${nsc} describe account AUTH --field nats.authorization | jq -r)"
            # test that AUTH account has `authorization.allowed_accounts` is a list of 2 accounts
            if [[ $(echo "$AUTH_ACCOUNT_AUTH_SETTINGS" | jq '.allowed_accounts | length') -ne 2 ]]; then
                echo "allowed_accounts does NOT contain exactly 2 entries."
                exit 1
            fi
            # test that AUTH account has `authorization.auth_users` is a list of 1 user
            HUB_AUTH_USER="$(${nsc} describe user -n auth -a AUTH --field sub | jq -r)"
            if [[ $(echo "$AUTH_ACCOUNT_AUTH_SETTINGS" | jq '.auth_users') == $HUB_AUTH_USER ]]; then
                echo "auth_user is NOT the expected user auth."
                exit 1
            fi

            ${natsCli} context save SYS_USER --nsc "nsc://HOLO/SYS/sys.creds"
            ${natsCli} -s "${natsServer}" stream ls --context SYS_USER
            # test that 1 stream exists
            ${natsCli} -s "${natsServer}" stream info --json ${workloadStreamName} --context SYS_USER
            # test that WORKLOAD stream *is* the single stream

            ${natsCli} -s "${natsServer}" micro ls --context SYS_USER
            # test that 1 service exists
            ${natsCli} -s "${natsServer}" micro info --json ${authServiceName} --context SYS_USER
            # test that AUTH service *is* the single service registered
          '';

        hostAuthTestScript =
          let
            natsServer = "nats://127.0.0.1:${builtins.toString nodes.host.holo.host-agent.nats.listenPort}";
          in
          pkgs.writeShellScript "cmd" ''
            set -xe

            WORKOAD_PKS=$(nsc list keys --users --account WORKLOAD 2>&1 | grep -E '^\|\s+auth' | awk '{print $4}')
            WORKLOD_PK_ARRAY=($WORKOAD_PKS)
            HOST_PUBKEY=$(echo "${WORKLOD_PK_ARRAY[0]}")

            ${natsCli} context save HOST_USER --nsc "nsc://HOLO/WORKLOAD/host_user_${HOST_PUBKEY}.creds"
            ${natsCli} context save SYS_USER --nsc "nsc://HOLO/SYS/sys_user_${HOST_PUBKEY}.creds"

            ${natsCli} -s "${natsServer}" stream ls --context SYS_USER
            ${natsCli} -s "${natsServer}" stream info --json ${workloadStreamName} --context SYS_USER

            ${natsCli} -s '${natsServer}' sub --stream "${workloadStreamName}" '${workloadStreamName}.>' --count=10 --context SYS_USER
            ${natsCli} -s '${natsServer}' pub "${workloadStreamName}.hello" '{"message":"hello"}' --js-domain ${hubJsDomain} --count=10 --context HOST_USER
          '';
      in
      ''
        with subtest("start the hub and run the hub auth test"):
          hub.start()
          hub.wait_for_unit("nats.service")
          hub.wait_for_open_port(port = ${builtins.toString nodes.hub.holo.nats-server.websocket.port}, timeout = 1)

          hub.wait_for_unit("caddy.service")
          hub.wait_for_open_port(port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 1)
          
          host.wait_for_unit('holo-orchestrator')
          hub.succeed("${hubAuthTestScript}")

        with subtest("start the host and run the host auth test"):
          host.start()
          host.wait_for_unit('holo-host-agent')
          sleep 30 # wait for the auth service to run and complete
          host.succeed("${hostAuthTestScript}", timeout = 10)

        with subtest("verify that holo-host-agent spins up leaf server and wait for it to be ready"):
          host.wait_for_unit('nats.service')
          host.wait_for_open_port(addr = "${nodes.hub.networking.fqdn}", port = ${builtins.toString nodes.hub.holo.nats-server.websocket.externalPort}, timeout = 10)
      '';
  }
)

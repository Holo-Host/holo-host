# dev container demo scenario, all assuming running `nix develop .#rust` before each (simplest to use direnv)
#
# 1. terminal 1: `just dev-cycle-logs-compat`
# 2. terminal 2: `just dev-hcterm`. press tab twice.
# 3. terminal 3: `just dev-install-app`
# 4. switch to terminal 2 and smash the  `r` key

state-file := "./ham.state"

# test-happ-url := "https://gist.github.com/steveej/5443d6d15395aa23081f1ee04712b2b3/raw/fdacb9b723ba83743567f2a39a8bfbbffb46b1f0/test-zome.bundle"
test-happ-url := "https://gist.github.com/steveej/5443d6d15395aa23081f1ee04712b2b3/raw/c82daf7f03ef459fa9ec4f28c8eeb9602596cc22/humm-earth-core-happ.happ"


# using the "just-testing" network-seed
# HUMM_HIVE_DNA_HASH := "uhC0k9QwFJiqMvUb8-0gZ2wv-9ccRtw-HiHLSssZ1LzTTIw9GAQEZ"

# using the "holo" network-seed
HUMM_HIVE_DNA_HASH := "uhC0kwENLeSuselWQJtywbYB1QyFK1d-ujmFFtxsq6CYY7_Ohri2u"

WORKLOAD_ID := "67d2ef2a67d4b619a54286c4"

help:
    just --list

holochain-container i:
    #!/usr/bin/env bash
    set -xeE
    result=$(nix build --no-link --print-out-paths --impure --expr "(builtins.getFlake \"git+file://${PWD}\").packages.\${builtins.currentSystem}.extra-container-holochain.override { index = {{i}}; }")
    "$result"/bin/container destroy
    # no need to keep it around if lair is destroyed with the container
    rm {{state-file}}.{{ i }} || :
    "$result"/bin/container create

    "$result"/bin/container start "holochain{{ i }}"
    while ! sudo systemctl -M "holochain{{ i }}" is-active holochain; do
        sleep 1
    done

holochain-container-destroy i:
    extra-container destroy holochain{{ i }}

ham i +args:
    cargo run --bin ham -- \
        --port "$((8000 + {{i}} ))" \
        --state-path {{state-file}}."{{i}}" \
        {{ args }}

ham-install i:
    just ham {{i}} install-and-init-happ \
        --happ $(nix build --no-link --print-out-paths -vL .\#holochain-zome-testing-happ)/happ.bundle \
        --network-seed "ham-test"

# make zomecalls to a holochain instance that was set up with `ham-test`
ham-zomecalls i zomecalls="holochain_zome_testing_0:get_registrations_pretty":
    just ham {{i}} zome-calls \
        --zome-calls "{{zomecalls}}"

# this will ensure a new holochain container at index i, and install the test zomes in it
ham-cycle i:
    #!/usr/bin/env bash
    set -xeE
    just holochain-container-destroy {{i}}
    just holochain-container {{i}}
    just ham-install {{i}}

dev-ham +args="":
    sudo machinectl -M dev-host shell .host /usr/bin/env ham --addr 127.0.0.1 --port 8000 --state-path /var/lib/holo-host-agent/workloads/{{WORKLOAD_ID}}/ham.state {{args}}

dev-ham-init-humm:
    just dev-ham zome-calls --zome-calls "humm_earth_core:init"

dev-ham-find-installed-app:
    # the installed-app-id is known because it's the hardcocded workload_id from `remote_cmds/mod.rs``
    just dev-ham find-installed-app {{WORKLOAD_ID}}


dev-hcterm bootstrap-url="http://dev-hub:50000":
    hcterm --bootstrap-url "{{bootstrap-url}}" --dna-hash "{{HUMM_HIVE_DNA_HASH}}"

dev-destroy:
    #!/usr/bin/env bash
    set -xeE
    extra-container destroy dev-hub
    extra-container destroy dev-host
    extra-container destroy dev-orch
    extra-container destroy dev-gw

dev-cycle:
    #!/usr/bin/env bash
    set -xeE
    nix build .\#extra-container-dev
    just dev-destroy
    # ./result/bin/container build
    ./result/bin/container create
    ./result/bin/container start dev-hub
    ./result/bin/container start dev-host
    ./result/bin/container start dev-orch
    ./result/bin/container start dev-gw


dev-cycle-logs-host-only:
    #!/usr/bin/env bash
    set -xeE
    nix build .\#extra-container-dev
    just dev-destroy
    # ./result/bin/container build
    ./result/bin/container create
    ./result/bin/container start dev-host
    just dev-logs

host-agent-remote +args="":
    #!/usr/bin/env bash
    set -xeE

    export RUST_BACKTRACE=1
    export RUST_LOG=mio=error,rustls=error,async_nats=error,trace

    cargo run --bin host_agent -- remote {{args}}


HOST_ID_STEVEEJ_HP := "5ba02d5ca17b416195e56e4f574644ba"
HOST_ID_DEV_HOST := "f0b9a2b7a95848389fdb43eda8139569"

host-agent-remote-hc desired-status +args="":
    #!/usr/bin/env bash
    set -xeE

    export RUST_BACKTRACE=1
    export RUST_LOG=mio=error,rustls=error,async_nats=error,trace

    # TODO(backlog): run a service on the host NATS instance that can be queried for the host-id
    # dev_machine_id="$(sudo machinectl shell dev-host /bin/sh -c "cat /etc/machine-id" | grep -oE '[a-z0-9]+')"

    just host-agent-remote holochain-dht-v1-workload \
        --workload-id-override "{{WORKLOAD_ID}}" \
        --desired-status "{{desired-status}}" \
        --happ-binary-url "{{test-happ-url}}" \
        --network-seed "holo" {{args}} \
        --holochain-feature-flags "unstable-functions,unstable-sharding,chc,unstable-countersigning" \
        --http-gw-enable

dev-host-host-agent-remote-hc desired-status +args="":
    #!/usr/bin/env bash
    set -xeE
    export NATS_URL="nats://dev-host"
    just host-agent-remote-hc {{desired-status}} \
        --host-id "{{HOST_ID_DEV_HOST}}" \
        --bootstrap-server-url "http://dev-hub:50000" \
        --signal-server-url "ws://dev-hub:50001" {{args}}
        # TODO: stun server?


dev-hub-host-agent-remote-hc desired-status subject="WORKLOAD.update" +args="":
    #!/usr/bin/env bash
    set -xeE
    export NATS_URL="wss://anon:anon@dev-hub:443"
    export NATS_SKIP_TLS_VERIFICATION_DANGER="true"
    just host-agent-remote-hc {{desired-status}} --subject-override {{subject}} --workload-only \
        --host-id "{{HOST_ID_DEV_HOST}}" \
        --bootstrap-server-url "http://dev-hub:50000" \
        --signal-server-url "ws://dev-hub:50001" \
        {{args}}

cloud-hub-host-agent-remote-hc desired-status subject="WORKLOAD.update" +args="":
    #!/usr/bin/env bash
    set -xeE
    export NATS_URL="wss://nats-server-0.holotest.dev:443"
    just host-agent-remote-hc {{desired-status}} --subject-override {{subject}} --workload-only \
        {{args}}

cloud-host-via-hub-host-agent-remote-hc desired-status subject="WORKLOAD.update" +args="":
    #!/usr/bin/env bash
    set -xeE
    export NATS_URL="wss://nats-server-0.holotest.dev:443"
    just host-agent-remote-hc {{desired-status}} --subject-override {{subject}} \
        --host-id "{{HOST_ID_STEVEEJ_HP}}" \
        {{args}}


# follows the logs for the applications services from the dev containers
# requires sudo because the containers log into the system journal
dev-logs +args="-f -n200":
    sudo journalctl -m \
    --unit holo-orchestrator \
    --unit holo-host-agent \
    --unit hc-http-gw \
    --unit holo-gateway \
    {{args}}

# (compat) follows the logs for the applications services from the dev containers
# requires sudo because the containers log into the system journal
dev-logs-compat +args="-f -n100":
    #!/usr/bin/env bash
    set -xeE
    (sudo machinectl shell dev-host /run/current-system/sw/sbin/journalctl --unit holo-host-agent {{args}}) &
    pid_hostagent=$!
    (sudo machinectl shell dev-orch /run/current-system/sw/sbin/journalctl --unit holo-orchestrator {{args}}) &
    pid_orchestrator=$!
    trap "kill $(jobs -pr)" SIGINT SIGTERM EXIT
    echo press CTRL+C **twice** to exit
    waitpid $pid_hostagent $pid_orchestrator


# re-create the dev containers and start following the relevant logs
dev-cycle-logs:
    just dev-cycle
    just dev-logs

# re-create the dev containers and start following the relevant logs in compat mode
dev-cycle-logs-compat:
    just dev-cycle
    just dev-logs-compat


dev-install-app:
    DONT_WAIT=true just dev-hub-host-agent-remote-hc reported WORKLOAD.add
    just dev-hub-host-agent-remote-hc running WORKLOAD.insert


cloud-install-app:
    DONT_WAIT=true just cloud-hub-host-agent-remote-hc reported WORKLOAD.add
    DONT_WAIT=true just cloud-hub-host-agent-remote-hc reported WORKLOAD.update
    just cloud-hub-host-agent-remote-hc running WORKLOAD.insert

cloud-uninstall-app:
    DONT_WAIT=true just cloud-hub-host-agent-remote-hc removed WORKLOAD.add
    DONT_WAIT=true just cloud-hub-host-agent-remote-hc removed WORKLOAD.insert


dev-http-gw-curl-hive host="http://dev-host:8090":
    #!/usr/bin/env bash
    set -xeE
    payload="$(base64 -i -w0 <<<'{ "hive_id":"MTc0MTA4ODg5NDA5Ni1iZmVjZGEwZDUxYTMxMjgz", "content_type": "hummhive-extension-story-v1" }')"
    curl --http1.1 -4v "{{host}}/{{HUMM_HIVE_DNA_HASH}}/{{WORKLOAD_ID}}/content/list_by_hive_link?payload=$payload"
    printf ""


dev-gw-curl-humm-hive:
    curl -4v "http://dev-gw/{{HUMM_HIVE_DNA_HASH}}/{{WORKLOAD_ID}}/humm_earth_core/init"


dev-hub-host-agent-remote-hc-humm desired-status subject="WORKLOAD.update" +args="":
    #!/usr/bin/env bash
    set -xeE
    export NATS_URL="wss://anon:anon@dev-hub:443"
    export NATS_SKIP_TLS_VERIFICATION_DANGER="true"
    just host-agent-remote-hc {{desired-status}} --subject-override {{subject}} --workload-only \
        --host-id "{{HOST_ID_DEV_HOST}}" \
        --bootstrap-server-url "https://bootstrap.holo.host" \
        --signal-server-url "wss://sbd.holo.host" \
        {{args}}


dev-install-humm-hive:
    DONT_WAIT=true just dev-hub-host-agent-remote-hc-humm reported WORKLOAD.add
    DONT_WAIT=true just dev-hub-host-agent-remote-hc-humm reported WORKLOAD.update
    just dev-hub-host-agent-remote-hc-humm running WORKLOAD.insert

dev-uninstall-humm-hive:
    DONT_WAIT=true just dev-hub-host-agent-remote-hc-humm deleted WORKLOAD.update
    just dev-hub-host-agent-remote-hc-humm deleted WORKLOAD.insert


dev-host-http-gw-remote-hive nats-url="nats://dev-hub":
    #!/usr/bin/env bash
    set -xeE
    # curl -4v "http://dev-host:8090/{{HUMM_HIVE_DNA_HASH}}/{{WORKLOAD_ID}}/humm_earth_core/init"
    # echo done

    payload="$(base64 -i -w0 <<<'{ "hive_id":"MTc0MTA4ODg5NDA5Ni1iZmVjZGEwZDUxYTMxMjgz", "content_type": "hummhive-extension-story-v1" }')"

    export NATS_URL="{{nats-url}}"
    just host-agent-remote hc-http-gw-req \
      --dna-hash {{HUMM_HIVE_DNA_HASH}} \
      --coordinatior-identifier {{WORKLOAD_ID}} \
      --zome-name "content" \
      --zome-fn-name "list_by_hive_link" \
      --payload "$payload"

state-file := "./ham.state"

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


dev-destroy:
    #!/usr/bin/env bash
    set -xeE
    extra-container destroy dev-hub
    extra-container destroy dev-host
    extra-container destroy dev-orch

dev-cycle:
    #!/usr/bin/env bash
    set -xeE
    nix build .\#extra-container-devhost
    just dev-destroy
    # ./result/bin/container build
    ./result/bin/container create
    ./result/bin/container start dev-hub
    ./result/bin/container start dev-host
    ./result/bin/container start dev-orch

host-agent-remote +args="":
    #!/usr/bin/env bash
    set -xeE

    export RUST_BACKTRACE=1
    export RUST_LOG=mio=error,rustls=error,async_nats=error,trace

    cargo run --bin host_agent -- remote {{args}}

host-agent-remote-hc desired-status +args="":
    #!/usr/bin/env bash
    set -xeE

    export RUST_BACKTRACE=1
    export RUST_LOG=mio=error,rustls=error,async_nats=error,trace

    # TODO(backlog): run a service on the host NATS instance that can be queried for the host-id
    # devhost_machine_id="$(sudo machinectl shell dev-host /bin/sh -c "cat /etc/machine-id" | grep -oE '[a-z0-9]+')"

    just host-agent-remote holochain-dht-v1-workload \
        --workload-id-override "67d2ef2a67d4b619a54286c4" \
        --desired-status "{{desired-status}}" \
        --host-id "f0b9a2b7a95848389fdb43eda8139569" \
        --happ-binary-url "https://gist.github.com/steveej/5443d6d15395aa23081f1ee04712b2b3/raw/fdacb9b723ba83743567f2a39a8bfbbffb46b1f0/test-zome.bundle" \
        --network-seed "just-testing" {{args}}

dev-host-host-agent-remote-hc desired-status:
    #!/usr/bin/env bash
    set -xeE
    export NATS_URL="nats://dev-host"
    just host-agent-remote-hc {{desired-status}}

dev-hub-host-agent-remote-hc desired-status subject="WORKLOAD.update" +args="":
    #!/usr/bin/env bash
    set -xeE
    export NATS_URL="wss://dev-hub:443"
    export NATS_SKIP_TLS_VERIFICATION_DANGER="true"
    just host-agent-remote-hc {{desired-status}} --subject-override {{subject}} --workload-only {{args}}

cloud-hub-host-agent-remote-hc desired-status subject="WORKLOAD.update":
    #!/usr/bin/env bash
    set -xeE
    export NATS_URL="wss://nats-server-0.holotest.dev:443"
    just host-agent-remote-hc {{desired-status}} --subject-override {{subject}} --workload-only


# follows the logs for the applications services from the dev containers
# requires sudo because the containers log into the system journal
dev-logs +args="-f -n200":
    sudo journalctl -m \
    --unit holo-orchestrator \
    --unit holo-host-agent \
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

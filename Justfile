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

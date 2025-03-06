state-file := "./ham.state"

# dev cycle for testing ham with a fresh container
ham-test:
    #!/usr/bin/env bash
    set -xeE
    result=$(nix build --no-link --print-out-paths .#extra-container-holochain)
    "$result"/bin/container destroy
    # no need to keep it around if lair is destroyed with the container
    rm {{state-file}} || :
    "$result"/bin/container create
    "$result"/bin/container start holochain

    while ! sudo systemctl -M holochain is-active holochain; do
        sleep 1
    done

    cargo run --bin ham -- \
        --port 8000 \
        --state-path {{state-file}} \
        install-and-init-happ \
                --happ $(nix build --no-link --print-out-paths -vL .\#holochain-zome-testing-happ)/happ.bundle \
                --network-seed "ham-test"

# make zomecalls to a holochain instance that was set up with `ham-test`
ham-test-zomecalls zomecalls="holochain_zome_testing_0:roundtrip:'hello zome world'":
    #!/usr/bin/env bash
    set -xeE
    cargo run --bin ham -- \
        --port 8000 \
        --state-path {{state-file}} \
        zome-calls \
            --zome-calls "{{zomecalls}}"

ham: ham-test ham-test-zomecalls

# holo-host main repository

This is an experiment to contain the code for all components in a single repository, also known as a monorepository.

## Repository Layout

The code is grouped by language or framework name.

### Nix

```
/flake.nix
/flake.lock
/nix/ /* [blueprint](https://github.com/numtide/blueprint)  set up underneath here. */
```

### Rust

```
/Cargo.toml
/Cargo.lock
/rust/ # all rust code lives here
/rust/common/Cargo.toml
/rust/common/src/lib.rs
/rust/holo-agentctl/Cargo.toml
/rust/holo-agentctl/src/main.rs
/rust/holo-agentd/Cargo.toml
/rust/holo-agentd/src/main.rs
/rust/holo-hqd/Cargo.toml
/rust/holo-hqd/src/main.rs
```

### Pulumi for Infrastructure-as-Code

Reusable Pulumi modules with examples

```
/pulumi/
```

## Continous Integration

The CI system is driven by [buildbot-nix](https://github.com/nix-community/buildbot-nix/).

## Licenses

Even when this repository is made publicly available, original code in this repository is explicitly stated to be unlicenced.
This means that this code cannot be modified or redistributed without explicit permission from the copyright holder, which are the authors in this repository.
This will change in the future when we have made the decision which open-source license to apply.

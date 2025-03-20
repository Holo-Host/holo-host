# holo-host main repository

This is an experiment to contain the code for all components in a single repository, also known as a monorepository.

Please run `sh setup-hooks.sh` to enforce correct naming convention for branches.

## Repository Layout

The code is grouped by language or framework name.

### Quickstart

Motivated by a shareable development experience, this repository provides

- [`nix develop .#rust`][nix develop] compatible shell environment containing a rust toolchain and other tools, including `nats` and `just`
- [`just`][just] compatible recipes via the Justfile

handily, `just` comes via the nix development shell as well.

```shell
nix develop .#rust
just
```

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
/rust/clients/
/rust/services/
/rust/hpos-hal/
/rust/netdiag/
/rust/util_libs/
```

### Pulumi for Infrastructure-as-Code

Reusable Pulumi modules with examples

```
/pulumi/
```

## Continous Integration

The CI system is driven by [buildbot-nix](https://github.com/nix-community/buildbot-nix/).

## Development and Conventions

### Formatting

This repo is configured with `treefmt-nix` which can be invoked via:

```
nix fmt
```

## Licenses

Even when this repository is made publicly available, original code in this repository is explicitly stated to be unlicenced.
This means that this code cannot be modified or redistributed without explicit permission from the copyright holder, which are the authors in this repository.
This will change in the future when we have made the decision which open-source license to apply.

[just]: https://just.systems/man/en/
[nix develop]: https://zero-to-nix.com/concepts/dev-env/

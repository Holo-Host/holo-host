# holo-host repository

This is an experiment is to contain all production components and their tests in a single repository, also known as a monorepository.

## Layout

```
/README.md
```

### Nix

```
/flake.nix
/flake.lock
/nix/ # [blueprint set up underneath here](https://github.com/numtide/blueprint)
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

**_?_**

/// This module contains a trait and types to model the lifecyle of contained
/// NixOS instances Even though it has been originally inspired by the
/// [extra-container](https://github.com/erikarvstedt/extra-container) project,
/// and the first concrete implementation will be around that, the model might
/// diverge from that and has a goal to be independent of that.
pub mod nixos_contain {
    pub trait NixosContain {
        fn create(&mut self, _: CreateArgs) -> anyhow::Result<CreateResult>;

        fn build(&mut self, _: BuildArgs) -> anyhow::Result<BuildResult>;

        fn list(&self, _: ListArgs) -> anyhow::Result<ListResult>;

        fn restart(&mut self, _: RestartArgs) -> anyhow::Result<RestartResult>;

        fn destroy(self, _: DestroyArgs) -> anyhow::Result<DestroyResult>;
    }

    pub struct CreateArgs {}
    pub struct CreateResult {}

    pub struct BuildArgs {}
    pub struct BuildResult {}

    pub struct ListArgs {}
    pub struct ListResult {}

    pub struct RestartArgs {}
    pub struct RestartResult {}

    pub struct DestroyArgs {}
    pub struct DestroyResult {}
}

///
pub mod extra_container {

    use crate::nixos_contain::*;

    pub struct ExtraContainer {}

    impl NixosContain for ExtraContainer {
        // extra-container create <container-config-file>
        //                        [--attr|-A attrPath]
        //                        [--nixpkgs-path|--nixos-path path]
        //                        [--start|-s | --restart-changed|-r]
        //                        [--ssh]
        //                        [--build-args arg...]

        //     <container-config-file> is a NixOS config file with container
        //     definitions like 'containers.mycontainer = { ... }'

        //     --attr | -A attrPath
        //       Select an attribute from the config expression

        //     --nixpkgs-path
        //       A nix expression that returns a path to the nixpkgs source
        //       to use for building the containers

        //     --nixos-path
        //       Like '--nixpkgs-path', but for directly specifying the NixOS source

        //     --start | -s
        //       Start all created containers
        //       Update running containers that have changed or restart them if '--restart-changed' was specified

        //     --update-changed | -u
        //       Update running containers with a changed system configuration by running
        //       'switch-to-configuration' inside the container.
        //       Restart containers with a changed container configuration

        //     --restart-changed | -r
        //       Restart running containers that have changed

        //     --ssh
        //       Generate SSH keys in /tmp and enable container SSH access.
        //       The key files remain after exit and are reused on subsequent runs.
        //       Unlocks the function 'cssh' in 'extra-container shell'.
        //       Requires container option 'privateNetwork = true'.

        //     --build-args arg...
        //       All following args are passed to nix-build.

        //     Example:
        //       extra-container create mycontainers.nix --restart-changed

        //       extra-container create mycontainers.nix --nixpkgs-path \
        //         'fetchTarball https://nixos.org/channels/nixos-unstable/nixexprs.tar.xz'

        //       extra-container create mycontainers.nix --start --build-args --builders 'ssh://worker - - 8'

        // echo <container-config> | extra-container create
        //     Read the container config from stdin

        //     Example:
        //       extra-container create --start <<EOF
        //         { containers.hello = { enableTun = true; config = {}; }; }
        //       EOF

        // extra-container create --expr|-E <container-config>
        //     Provide container config as an argument

        // extra-container create <store-path>
        //     Create containers from <store-path>/etc

        //     Examples:
        //       Create from nixos system derivation
        //       extra-container create /nix/store/9h..27-nixos-system-foo-18.03

        //       Create from nixos etc derivation
        //       extra-container create /nix/store/32..9j-etc
        fn create(&mut self, _: CreateArgs) -> anyhow::Result<CreateResult> {
            todo!()
        }

        // extra-container shell ...
        //     Start a container shell session.
        //     See the README for a complete documentation.
        //     Supports all arguments from 'create'

        //     Extra arguments:
        //       --run <cmd> <arg>...
        //         Run command in shell session and exit
        //         Must be the last option given
        //       --no-destroy|-n
        //         Do not destroy shell container before and after running
        //       --destroy|-d
        //         If running inside an existing shell session, force container to
        //         be destroyed before and after running

        //     Example:
        //       extra-container shell -E '{ containers.demo.config = {}; }'

        // extra-container build ...
        //     Build the container config and print the resulting NixOS system etc path

        //     This command can be used like 'create', but options related
        //     to starting are not supported
        fn build(&mut self, _: BuildArgs) -> anyhow::Result<BuildResult> {
            todo!()
        }

        // extra-container list
        //     List all extra containers
        fn list(&self, _: ListArgs) -> anyhow::Result<ListResult> {
            todo!()
        }

        // extra-container restart <container>...
        //     Fixes the broken restart command of nixos-container (nixpkgs issue #43652)
        fn restart(&mut self, _: RestartArgs) -> anyhow::Result<RestartResult> {
            todo!()
        }

        // extra-container destroy <container-name>...
        //     Destroy containers

        // extra-container destroy <args for create/shell>...
        //     Destroy the containers defined by the args for command `create` or `shell` (see above).
        //     For this to work, the first arg after `destroy` must start with one of the
        //     following three characters: ./-

        //     Example:
        //       extra-container destroy ./containers.nix

        // extra-container destroy --all|-a
        //     Destroy all extra containers
        fn destroy(self, _: DestroyArgs) -> anyhow::Result<DestroyResult> {
            todo!()
        }

        // extra-container <cmd> <arg>...
        //     All other commands are forwarded to nixos-container
    }
}

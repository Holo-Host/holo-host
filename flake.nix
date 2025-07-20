{
  description = "holo-host monorepository";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-24.11";
    nixpkgs-2405.url = "github:NixOS/nixpkgs?ref=nixos-24.05";
    nixpkgs-2411.url = "github:NixOS/nixpkgs?ref=nixos-24.11";
    nixpkgs-unstable.url = "github:NixOS/nixpkgs?ref=nixos-unstable";
    blueprint.url = "github:numtide/blueprint";
    blueprint.inputs.nixpkgs.follows = "nixpkgs";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
    nixago.url = "github:jmgilman/nixago";
    nixago.inputs.nixpkgs.follows = "nixpkgs";

    srvos.url = "github:nix-community/srvos";
    srvos.inputs.nixpkgs.follows = "nixpkgs";
    disko.url = "github:nix-community/disko";
    disko.inputs.nixpkgs.follows = "nixpkgs";

    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    extra-container.url = "github:erikarvstedt/extra-container";
    extra-container.inputs.nixpkgs.follows = "nixpkgs";

    # Add support for multiple holochain versions
    holonix_0_5 = {
      url = "github:holochain/holonix?ref=main-0.5";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        crane.follows = "crane";
        rust-overlay.follows = "rust-overlay";
      };
    };
    holonix_0_4 = {
      url = "github:holochain/holonix?ref=main-0.4";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        crane.follows = "crane";
        rust-overlay.follows = "rust-overlay";
      };
    };
    holonix_0_3 = {
      url = "github:holochain/holonix?ref=main-0.3";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        crane.follows = "crane";
        rust-overlay.follows = "rust-overlay";
      };
    };

    hc-http-gw = {
      url = "github:holochain/hc-http-gw";
      inputs = {
        holonix.follows = "holonix_0_4";
        nixpkgs.follows = "nixpkgs";
        rust-overlay.follows = "rust-overlay";
      };
    };
  };

  outputs =
    inputs:
    let
      portAlloc = import ./nix/lib/port-allocation.nix { lib = inputs.nixpkgs.lib; };
      pkgs = import inputs.nixpkgs { system = "x86_64-linux"; };
      blueprintOutputs = inputs.blueprint {
        inherit inputs;
        prefix = "nix/";
        nixpkgs.config.allowUnfree = true;
      };
    in
    blueprintOutputs // {
      checks.x86_64-linux.port-allocation = pkgs.runCommand "port-allocation-check" {} ''
        echo "${builtins.concatStringsSep "\n" portAlloc.testResults}" > $out
      '';
      checks.x86_64-linux.extra-container-build-validation = 
        blueprintOutputs.packages.x86_64-linux.extra-container-holochain.tests.build-validation;
      checks.x86_64-linux.extra-container-error-detection = 
        blueprintOutputs.packages.x86_64-linux.extra-container-holochain.tests.error-detection;
      checks.x86_64-linux.extra-container-version-validation = 
        blueprintOutputs.packages.x86_64-linux.extra-container-holochain.tests.version-validation;
      checks.x86_64-linux.holo-agent-integration-nixos = 
        import ./nix/checks/holo-agent-integration-nixos.nix { 
          inherit inputs; 
          flake = blueprintOutputs; 
          pkgs = pkgs; 
          system = "x86_64-linux"; 
        };
      checks.x86_64-linux.holo-nsc-proxy = 
        import ./nix/checks/holo-nsc-proxy.nix { 
          inherit inputs; 
          flake = blueprintOutputs; 
          pkgs = pkgs; 
          system = "x86_64-linux"; 
        };
      checks.x86_64-linux.holo-distributed-auth = 
        import ./nix/checks/holo-distributed-auth.nix { 
          inherit inputs; 
          flake = blueprintOutputs; 
          pkgs = pkgs; 
          system = "x86_64-linux"; 
        };
    };
}

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
    inputs.blueprint {
      inherit inputs;
      prefix = "nix/";
      nixpkgs.config.allowUnfree = true;
    };
}

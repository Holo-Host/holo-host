# Package for the host agent auth setup script
# This script sets up authentication and credential management on host agents

{
  inputs,
  system,
  flake,
  pkgs,
  nixpkgs ? inputs.nixpkgs-2411,
  ...
}:

let
  script = builtins.readFile ../../../scripts/host_agent_auth_setup.sh;
in
pkgs.writeScriptBin "host-agent-auth-setup" script 
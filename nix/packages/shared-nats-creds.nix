/*
This file is a package that creates a secure package of shared credentials
for distribution to host agents via the Nix store.

This can be used to securely distribute NATS credentials from the orchestrator
or NATS server to host agents without using network transfers.

Usage:
$ nix build --print-out-paths .#packages.x86_64-linux.shared-creds

The output will be a directory containing the shared credentials with
proper integrity verification and metadata.
*/

{
  inputs,
  system,
  flake,
  pkgs,
  nixpkgs ? inputs.nixpkgs-2411,
  # Source directory containing the shared credentials
  sourceDir ? null,
  # Optional: expected hash for integrity verification
  expectedHash ? null,
  # Optional: description for the derivation
  description ? "Shared NATS credentials for host agents"
}:

let
  lib = nixpkgs.lib;
  
  # Script to verify and package credentials
  packageScript = pkgs.writeShellScript "package-shared-creds" ''
    set -euo pipefail
    
    echo "Packaging shared credentials from: $sourceDir"
    
    # Verify source directory exists
    if [ ! -d "$sourceDir" ]; then
      echo "ERROR: Source directory $sourceDir does not exist"
      exit 1
    fi
    
    # Check for required credential files
    required_files=(
      "HOLO.jwt"
      "SYS.jwt"
      "auth-guard.creds"
    )
    
    for file in "''${required_files[@]}"; do
      if [ ! -f "$sourceDir/$file" ]; then
        echo "WARNING: Required credential file $file not found in $sourceDir"
      fi
    done
    
    # Calculate hash of all files for integrity verification
    echo "Calculating integrity hash..."
    FILES_HASH=$(find "$sourceDir" -type f -exec sha256sum {} \; | sort | sha256sum | cut -d' ' -f1)
    echo "Files hash: $FILES_HASH"
    
    # Verify against expected hash if provided
    if [ -n "${expectedHash:-}" ]; then
      if [ "$FILES_HASH" != "${expectedHash:-}" ]; then
        echo "ERROR: Integrity check failed!"
        echo "Expected: ${expectedHash:-}"
        echo "Actual: $FILES_HASH"
        exit 1
      fi
      echo "Integrity verification passed"
    fi
    
    # Create output directory
    mkdir -p $out
    
    # Copy all files from source directory
    cp -r "$sourceDir"/* "$out/"
    
    # Create metadata file with hash and timestamp
    cat > "$out/.metadata" << EOF
    # Shared Credentials Metadata
    # Generated: $(date -u --iso-8601=seconds)
    # Source: $sourceDir
    # Integrity Hash: $FILES_HASH
    # Expected Hash: ${if expectedHash != null then expectedHash else "not provided"}
    # Files:
    $(find "$sourceDir" -type f -exec basename {} \; | sort)
    EOF
    
    # Set restrictive permissions on output
    chmod -R 600 "$out"/*
    chmod 644 "$out/.metadata"
    
    echo "Shared credentials packaged successfully to: $out"
    echo "Integrity hash: $FILES_HASH"
  '';

  # The actual derivation
  sharedCredsPackage = pkgs.runCommand "shared-nats-creds" {
    inherit description;
    buildInputs = with pkgs; [ coreutils findutils gnugrep ];
    sourceDir = if sourceDir != null then toString sourceDir else "";
    expectedHash = if expectedHash != null then expectedHash else "";
  } ''
    ${packageScript}
  '';

in sharedCredsPackage 
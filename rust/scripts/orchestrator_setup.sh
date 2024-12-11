# --------
# NB: This setup expects the `nats` and the `nsc` binarys to be locally installed and accessible.  This script will verify that they both exist locally before running setup commnds.

# Script Overview:
# This script is responsible ser setting up the "Operator Chain of Trust" (eg: O/A/U) authentication pattern on the Orchestrator Hub.

# Operator Creation:
# The operator is named "HOLO", with a system account (SYS), two signing keys, and the JWT server URL set to nats://0.0.0.0:4222.

# Account Creation:
# Two accounts, "ORCHESTRATOR" and "HPOS", are created with JetStream enabled.
# Each account has a signing key with a randomly generated role name, scoped to allow only users assigned to the signing key to publish and subscribe to their respective streams.

# User Creation:
# An admin user is created in the "ORCHESTRATOR" account.

# JWT Generation:
# JWT files are generated for the operator and both accounts, saved in the jwt_output/ directory.

# Input Vars:
#   - OPERATOR_NAME
#   - SYS_ACCOUNT
#   - ACCOUNT_JWT_SERVER
#   - ORCHESTRATOR_ACCOUNT
#   - HPOS_ACCOUNT
#   - OUTPUT_DIR
#   - RESOLVER_FILE

# Output:
# JWT Files: operator.jwt, orchestrator_account.jwt, and hpos_account.jwt in the jwt_output/ directory.
# Admin User: An admin user under the "ORCHESTRATOR" account.
# --------

#!/bin/bash

set -e  # Exit on any error

# Check for required commands
for cmd in nsc nats
do
  echo "Executing command: $cmd --version"
  if command -v "$cmd" &> /dev/null
  then
    $cmd --version
  else
    echo "Command '$cmd' not found."
  fi
done

# Variables
OPERATOR_NAME="HOLO"
SYS_ACCOUNT="SYS"
ACCOUNT_JWT_SERVER="nats://0.0.0.0:4222"
ORCHESTRATOR_ACCOUNT="ORCHESTRATOR"
HPOS_ACCOUNT="HPOS"
OUTPUT_DIR="jwt_output"
RESOLVER_FILE="main-resolver.conf"

# Create output directory
mkdir -p $OUTPUT_DIR

# Step 1: Create Operator with SYS account and two signing keys
nsc add operator --name $OPERATOR_NAME --sys --generate-signing-key
nsc edit operator --require-signing-keys --account-jwt-server-url $ACCOUNT_JWT_SERVER
nsc edit operator --sk generate 

# Step 2: Create ORCHESTRATOR Account with JetStream and scoped signing key
nsc add account --name $ORCHESTRATOR_ACCOUNT
nsc edit account --name $ORCHESTRATOR_ACCOUNT --js-streams -1 --js-consumer -1 --js-mem-storage 1G --js-disk-storage 5G --js-streams 10 --js-consumer 100
SIGNING_KEY_ORCHESTRATOR="$(echo "$(nsc edit account -n $ORCHESTRATOR_ACCOUNT --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
ROLE_NAME_ORCHESTRATOR=$(mktemp -u XXXXXX)
nsc edit signing-key --sk $SIGNING_KEY_ORCHESTRATOR --role $ROLE_NAME_ORCHESTRATOR --allow-pub "ORCHESTRATOR.>" --allow-sub "ORCHESTRATOR.>" --allow-pub-response

# Step 3: Create HPOS Account with JetStream and scoped signing key
nsc add account --name $HPOS_ACCOUNT
nsc edit account --name $HPOS_ACCOUNT --js-streams -1 --js-consumer -1 --js-mem-storage 1G --js-disk-storage 5G --js-streams 10 --js-consumer 100
SIGNING_KEY_HPOS="(echo "$(nsc edit account -n $HPOS_ACCOUNT --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
ROLE_NAME_HPOS=$(mktemp -u XXXXXX)
nsc edit signing-key --sk $SIGNING_KEY_HPOS --role $ROLE_NAME_HPOS --allow-pub "HPOS.>" --allow-sub "HPOS.>" --allow-pub-response

# Step 4: Create Users "admin" and "noauth" in ORCHESTRATOR Account
nsc add user --name admin --account $ORCHESTRATOR_ACCOUNT
nsc add user --name noauth --account $ORCHESTRATOR_ACCOUNT

# Step 5: Generate JWT files
nsc describe operator --raw --output-file $OUTPUT_DIR/operator.jwt
nsc describe account --name $ORCHESTRATOR_ACCOUNT --raw --output-file $OUTPUT_DIR/orchestrator_account.jwt
nsc describe account --name $HPOS_ACCOUNT --raw --output-file $OUTPUT_DIR/hpos_account.jwt

# Step 6: Generate Resolver Config
nsc generate config --nats-resolver --sys-account $SYS_ACCOUNT --force --config-file $RESOLVER_FILE

# Step 7: Push credentials to NATS server
nsc push -A

echo "Setup complete. JWTs and resolver file are in the $OUTPUT_DIR/ directory."

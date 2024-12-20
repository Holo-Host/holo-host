#!/usr/bin/env bash
# shellcheck disable=SC2005,SC2086

# --------
# NB: This setup expects the `nats` and the `nsc` binarys to be locally installed and accessible.  This script will verify that they both exist locally before running setup commnds.

# Script Overview:
# This script is responsible ser setting up the "Operator Chain of Trust" (eg: O/A/U) authentication pattern on the Orchestrator Hub.

# Operator Creation:
# The operator is generated and named "HOLO" and a system account is generated and named SYS.  Both are assigned two signing keys and are associated with the JWT server.  The JWT server URL set to nats://0.0.0.0:4222.

# Account Creation:
# Two accounts, named "ADMIN" and "WORKLOAD", are created with JetStream enabled.  Both are associated with the HOLO Operator.
# Each account has a signing key with a randomly generated role name, which is assigned scoped permissions to allow only users assigned to the signing key to publish and subscribe to their respective streams.

# User Creation:
# One user named "admin" is created under the "ADMIN" account.
# One user named "orchestrator" is created under the "WORKLOAD" account.

# JWT Generation:
# JWT files are generated for the operator and both accounts, saved in the jwt_output/ directory.

# Input Vars:
#   - OPERATOR_NAME
#   - SYS_ACCOUNT
#   - ACCOUNT_JWT_SERVER
#   - WORKLOAD_ACCOUNT
#   - ADMIN_ACCOUNT
#   - JWT_OUTPUT_DIR
#   - RESOLVER_FILE

# Output:
# One Operator: HOLO
# Two accounts: HOLO/ADMIN & `HOLO/WORKLOAD`
# Two Users: /HOLO/ADMIN/admin & HOLO/WORKLOAD/orchestrator
# JWT Files: holo-operator.jwt, sys_account.jwt, admin_account.jwt and workload_account.jwt in the `jwt_output` directory.
# --------

set -e # Exit on any error

# Check for required commands
for cmd in nsc nats; do
  echo "Executing command: $cmd --version"
  if command -v "$cmd" &>/dev/null; then
    $cmd --version
  else
    echo "Command '$cmd' not found."
  fi
done

# Variables
OPERATOR_NAME="HOLO"
SYS_ACCOUNT="SYS"
ACCOUNT_JWT_SERVER="nats://143.244.144.52:4222"
OPERATOR_SERVICE_URL="nats://143.244.144.52:4222"
WORKLOAD_ACCOUNT="WORKLOAD"
ADMIN_ACCOUNT="ADMIN"
JWT_OUTPUT_DIR="jwt_output"
RESOLVER_FILE="main-resolver.conf"

# Create output directory
mkdir -p $JWT_OUTPUT_DIR

# Step 1: Create Operator with SYS account and two signing keys
nsc add operator --name $OPERATOR_NAME --sys --generate-signing-key
nsc edit operator --require-signing-keys --account-jwt-server-url $ACCOUNT_JWT_SERVER --service-url $OPERATOR_SERVICE_URL
nsc edit operator --sk generate

# Step 2: Create ADMIN_Account with JetStream and scoped signing key
nsc add account --name $ADMIN_ACCOUNT
nsc edit account --name $ADMIN_ACCOUNT --js-streams -1 --js-consumer -1 --js-mem-storage 1G --js-disk-storage 5G
SIGNING_KEY_ADMIN="$(echo "$(nsc edit account -n $ADMIN_ACCOUNT --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
ROLE_NAME_ADMIN="admin_role"
nsc edit signing-key --sk $SIGNING_KEY_ADMIN --role $ROLE_NAME_ADMIN --allow-pub "ADMIN_>" --allow-sub "ADMIN_>" --allow-pub-response

# Step 3: Create WORKLOAD Account with JetStream and scoped signing key
nsc add account --name $WORKLOAD_ACCOUNT
nsc edit account --name $WORKLOAD_ACCOUNT --js-streams -1 --js-consumer -1 --js-mem-storage 1G --js-disk-storage 5G
SIGNING_KEY_WORKLOAD="$(echo "$(nsc edit account -n $WORKLOAD_ACCOUNT --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
ROLE_NAME_WORKLOAD="workload-role"
nsc edit signing-key --sk $SIGNING_KEY_WORKLOAD --role $ROLE_NAME_WORKLOAD --allow-pub "WORKLOAD.>" --allow-sub "WORKLOAD.>" --allow-pub-response

# Step 4: Create User "orchestrator" in ADMIN Account // noauth
nsc add user --name admin --account $ADMIN_ACCOUNT

# Step 5: Create User "orchestrator" in WORKLOAD Account
nsc add user --name orchestrator --account $WORKLOAD_ACCOUNT

# Step 6: Generate JWT files
nsc describe operator --raw --output-file $JWT_OUTPUT_DIR/holo_operator.jwt
nsc describe account --name SYS --raw --output-file $JWT_OUTPUT_DIR/sys_account.jwt
nsc describe account --name $WORKLOAD_ACCOUNT --raw --output-file $JWT_OUTPUT_DIR/workload_account.jwt
nsc describe account --name $ADMIN_ACCOUNT --raw --output-file $JWT_OUTPUT_DIR/admin_account.jwt

# Step 7: Generate Resolver Config
nsc generate config --nats-resolver --sys-account $SYS_ACCOUNT --force --config-file $RESOLVER_FILE

# Step 8: Push credentials to NATS server
nsc push -A

echo "Setup complete. JWTs and resolver file are in the $JWT_OUTPUT_DIR/ directory."

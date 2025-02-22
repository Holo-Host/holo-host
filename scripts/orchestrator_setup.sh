#!/usr/bin/env bash
# shellcheck disable=SC2005,SC2086

# --------
# NB: This setup expects the `nats` and the `nsc` binarys to be locally installed and accessible.  This script will verify that they both exist locally before running setup commnds.

# Script Overview:
# This script is responsible ser setting up the "Operator Chain of Trust" (eg: O/A/U) authentication pattern on the Orchestrator Hub.

# Operator Creation:
# The operator is generated and named "HOLO" and a system account is generated and named SYS.  Both are assigned two signing keys and are associated with the JWT server.  The JWT server URL set to nats://0.0.0.0:4222.

# Account Creation:
# Two accounts, named "ADMIN" and "HPOS", are created with JetStream enabled.  Both are associated with the HOLO Operator.
# Each account has a signing key with a randomly generated role name, which is assigned scoped permissions to allow only users assigned to the signing key to publish and subscribe to their respective streams.

# User Creation:
# One user named "admin" is created under the "ADMIN" account.
# One user named "orchestrator" is created under the "HPOS" account.

# JWT Generation:
# JWT files are generated for the operator and both accounts, saved in the jwt_output/ directory.

# Input Vars:
#   - OPERATOR_NAME
#   - SYS_ACCOUNT
#   - ACCOUNT_JWT_SERVER
#   - HPOS_ACCOUNT
#   - ADMIN_ACCOUNT
#   - JWT_OUTPUT_DIR
#   - RESOLVER_FILE

# Output:
# One Operator: HOLO
# Two accounts: HOLO/ADMIN & `HOLO/HPOS`
# One Users: /HOLO/ADMIN/admin
# JWT Files: holo-operator.jwt, sys_account.jwt, admin_account.jwt and hpos_account.jwt in the `jwt_output` directory.
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
NATS_SERVER_HOST=$1
NATS_PORT="4222"
OPERATOR_SERVICE_URL="nats://{$NATS_SERVER_HOST}:$NATS_PORT"
ACCOUNT_JWT_SERVER="nats://{$NATS_SERVER_HOST}:$NATS_PORT"
OPERATOR_NAME="HOLO"
SYS_ACCOUNT="SYS"
HPOS_ACCOUNT="HPOS"
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
ADMIN_SIGNING_KEY="$(echo "$(nsc edit account -n $ADMIN_ACCOUNT --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
ADMIN_ROLE_NAME="admin-role"
nsc edit signing-key --sk $ADMIN_SIGNING_KEY --role $ADMIN_ROLE_NAME --allow-pub "ADMIN.>","WORKLOAD.>","\$JS.>","\$SYS.>","_INBOX.>","_INBOX_*.>","*._WORKLOAD_INBOX.>" --allow-sub "ADMIN.>""WORKLOAD.>","\$JS.>","\$SYS.>","_INBOX.>","_INBOX_*.>","ORCHESTRATOR._WORKLOAD_INBOX.>" --allow-pub-response

# Step 3: Create HPOS Account with JetStream and scoped signing key
nsc add account --name $HPOS_ACCOUNT
nsc edit account --name $HPOS_ACCOUNT --js-streams -1 --js-consumer -1 --js-mem-storage 1G --js-disk-storage 5G
HPOS_SIGNING_KEY="$(echo "$(nsc edit account -n $HPOS_ACCOUNT --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
WORKLOAD_ROLE_NAME="workload-role"
nsc edit signing-key --sk $HPOS_SIGNING_KEY --role $WORKLOAD_ROLE_NAME --allow-pub "WORKLOAD.>","{{tag(pubkey)}}._WORKLOAD_INBOX.>","\$JS.API>" --allow-sub "WORKLOAD.{{tag(pubkey)}}.*","{{tag(pubkey)}}._WORKLOAD_INBOX.>","\$JS.API>" --allow-pub-response

# Step 4: Create User "admin" in ADMIN Account
nsc add user --name admin --account $ADMIN_ACCOUNT

# Step 5: Export/Import WORKLOAD Service Stream between ADMIN and HPOS accounts
# Share orchestrator (as admin user) workload streams with host
nsc add export --name "WORKLOAD_SERVICE" --subject "WORKLOAD.>" --account ADMIN
nsc add import --src-account ADMIN --name "WORKLOAD_SERVICE" --remote-subject "WORKLOAD.>" --local-subject "WORKLOAD.>" --account HPOS
# Share host workload streams with orchestrator (as admin user)
nsc add export --name "WORKLOAD_SERVICE" --subject "WORKLOAD.>" --account HPOS
nsc add import --src-account HPOS --name "WORKLOAD_SERVICE" --remote-subject "WORKLOAD.>" --local-subject "WORKLOAD.>" --account ADMIN

# Step 6: Generate JWT files
nsc describe operator --raw --output-file $JWT_OUTPUT_DIR/holo_operator.jwt
nsc describe account --name SYS --raw --output-file $JWT_OUTPUT_DIR/sys_account.jwt
nsc describe account --name $HPOS_ACCOUNT --raw --output-file $JWT_OUTPUT_DIR/hpos_account.jwt
nsc describe account --name $ADMIN_ACCOUNT --raw --output-file $JWT_OUTPUT_DIR/admin_account.jwt

# Step 7: Generate Resolver Config
nsc generate config --nats-resolver --sys-account $SYS_ACCOUNT --force --config-file $RESOLVER_FILE

echo "Setup complete. JWTs and resolver file are in the $JWT_OUTPUT_DIR/ directory."
echo "!! Don't forget to start the NATS server and push the credentials to the server with 'nsc push -A' !!"

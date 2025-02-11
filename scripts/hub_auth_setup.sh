#!/usr/bin/env bash
# shellcheck disable=SC2005,SC2086

# --------
# NB: This setup expects the `nats` and the `nsc` binaries to be locally installed and accessible.  This script will verify that they both exist locally before running setup commnds.

# Script Overview:
# This script is responsible ser setting up the "Operator Chain of Trust" (eg: O/A/U) authentication pattern on the Orchestrator Hub.  It also instantiates the distributed auth-callout on the orchestrator NATS server hub for the AUTH account.

# Operator Creation:
# The operator is generated and named "HOLO".  The JWT server URL set to nats://0.0.0.0:4222.

# Account Creation:
# Four accounts, named "ADMIN", "SYS", "AUTH", and "WORKLOAD" are all associated with the HOLO Operator.  The "ADMIN", "AUTH" and "WORKLOAD" accounts are created with JetStream enabled.
# Each account has a signing key, of which both the ADMIN and WORKLOAD signing keys are scoped by an assigned role name.  This ensures that only the users assigned to the signing key role inheirt their scoped permissions.

# User Creation:
# Four users:
#  - user named "admin" is created under the "ADMIN" account.
#  - user named "orchestrator_auth" is created under the "AUTH" account.
#  - user named "sys" is created under the "SYS" account.
#  - user named "auth_guard" is created under the "AUTH" account.

# JWT Generation:
# JWT files are generated for the operator and sys account and are saved in the shared_creds/ directory.

# Input Vars:
#   - OPERATOR
#   - SYS_ACCOUNT
#   - NATS_LISTENING_PORT
#   - ACCOUNT_JWT_SERVER
#   - OPERATOR_SERVICE_URL
#   - ADMIN_ACCOUNT
#   - ADMIN_USER
#   - AUTH_ACCOUNT
#   - ORCHESTRATOR_AUTH_USER
#   - AUTH_GUARD_USER
#   - WORKLOAD_ACCOUNT 
#   - SHARED_CREDS_PATH
#   - LOCAL_CREDS_PATH 
#   - RESOLVER_FILE

# Output:
# One Operator: HOLO
# Four Accounts: HOLO/ADMIN & HOLO/SYS (SYS is automated) & HOLO/AUTH & HOLO/WORKLOAD
# Four Users: /HOLO/ADMIN/admin & HOLO/SYS/sys (sys is automated) & /HOLO/AUTH/orchestrator_auth & HOLO/AUTH/auth_guard
# Private Files (to only be stored local to Orchestrator): `orchestrator_auth.creds` in the `local_creds/` directory.
# Shared Files (to be exported to HPOSs): HOLO.jwt, SYS.jwt, `auth_guard.creds` in the `shared_creds/` directory.
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
NATS_LISTENING_PORT=$2 # "4222"
SHARED_CREDS_PATH=$3 # "/shared_creds"
LOCAL_CREDS_PATH=$4 # "/local_creds"
OPERATOR_SERVICE_URL="nats://{$NATS_SERVER_HOST}:$NATS_LISTENING_PORT"
ACCOUNT_JWT_SERVER="nats://{$NATS_SERVER_HOST}:$NATS_LISTENING_PORT"
RESOLVER_FILE="main-resolver.conf"
OPERATOR="HOLO"
SYS_ACCOUNT="SYS"
ADMIN_ACCOUNT="ADMIN"
ADMIN_USER="admin"
AUTH_ACCOUNT="AUTH"
ORCHESTRATOR_AUTH_USER="orchestrator_auth"
AUTH_GUARD_USER="auth_guard"
WORKLOAD_ACCOUNT="WORKLOAD"

# Create output directory when it doesn't already exist
if [ ! -d "$SHARED_CREDS_PATH" ]; then
    echo "The shared output dir does not exist. Creating $SHARED_CREDS_PATH."
    mkdir -p $SHARED_CREDS_PATH
    echo "Shared output dir created successfully."
else
    echo "Shared output dir exists."
fi

if [ ! -d "$LOCAL_CREDS_PATH" ]; then
    echo "The local output dir does not exist. Creating $LOCAL_CREDS_PATH."
    mkdir -p $LOCAL_CREDS_PATH
    echo "Local output dir created successfully."
else
    echo "Local output dir exists."
fi

function extract_signing_key() {
  sk=$2
  name=$1
  seed_file_path="$HOME/.local/share/nats/nsc/keys/keys/${sk:0:1}/${sk:1:2}/${sk}.nk"
  echo "coping file over to '$LOCAL_CREDS_PATH/${name}_SK.nk'"
  cp "$seed_file_path" "$LOCAL_CREDS_PATH/${name}_SK.nk"
}

# Step 1: Create Operator with SYS account and two signing keys
nsc add operator --name $OPERATOR --sys --generate-signing-key
nsc edit operator --require-signing-keys --account-jwt-server-url $ACCOUNT_JWT_SERVER --service-url $OPERATOR_SERVICE_URL
nsc edit operator --sk generate

# Step 2: Create ADMIN_Account with JetStream and scoped signing key
nsc add account --name $ADMIN_ACCOUNT
nsc edit account --name $ADMIN_ACCOUNT --js-streams -1 --js-consumer -1 --js-mem-storage 1G --js-disk-storage 5G --conns -1 --leaf-conns -1

ADMIN_SK="$(echo "$(nsc edit account -n $ADMIN_ACCOUNT --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
ADMIN_ROLE_NAME="admin_role"
nsc edit signing-key --sk $ADMIN_SK --role $ADMIN_ROLE_NAME --allow-pub "ADMIN.>","AUTH.>","WORKLOAD.>","\$JS.API.>","\$SYS.>","_INBOX.>","_INBOX_*.>","*._WORKLOAD_INBOX.>","_AUTH_INBOX_*.>" --allow-sub "ADMIN.>","AUTH.>","WORKLOAD.>","\$JS.API.>","\$SYS.>","_INBOX.>","_INBOX_*.>","ORCHESTRATOR._WORKLOAD_INBOX.>","_AUTH_INBOX_ORCHESTRATOR.>" --allow-pub-response

# Step 3: Create AUTH with JetStream with non-scoped signing key
nsc add account --name $AUTH_ACCOUNT
nsc edit account --name $AUTH_ACCOUNT --sk generate --js-streams -1 --js-consumer -1 --js-mem-storage 1G --js-disk-storage 5G --conns -1 --leaf-conns -1
AUTH_ACCOUNT_PUBKEY=$(nsc describe account $AUTH_ACCOUNT --field sub | jq -r)
AUTH_SK_ACCOUNT_PUBKEY=$(nsc describe account $AUTH_ACCOUNT --field 'nats.signing_keys[0]' | tr -d '"')

# Step 4: Create WORKLOAD Account with JetStream and scoped signing keys
nsc add account --name $WORKLOAD_ACCOUNT
nsc edit account --name $WORKLOAD_ACCOUNT --js-streams -1 --js-consumer -1 --js-mem-storage 1G --js-disk-storage 5G --conns -1 --leaf-conns -1
WORKLOAD_SK="$(echo "$(nsc edit account -n $WORKLOAD_ACCOUNT --sk generate 2>&1)" | grep -oP "signing key\s*\K\S+")"
WORKLOAD_ROLE_NAME="workload_role"
nsc edit signing-key --sk $WORKLOAD_SK --role $WORKLOAD_ROLE_NAME --allow-pub "WORKLOAD.>","{{tag(pubkey)}}._WORKLOAD_INBOX.>" --allow-sub "WORKLOAD.{{tag(pubkey)}}.*","{{tag(pubkey)}}._WORKLOAD_INBOX.>" --allow-pub-response

# Step 5: Create Orchestrator User in ADMIN Account
nsc add user --name $ADMIN_USER --account $ADMIN_ACCOUNT -K $ADMIN_ROLE_NAME

# Step 6: Create Orchestrator User in AUTH Account (used in auth-callout service)
nsc add user --name $ORCHESTRATOR_AUTH_USER --account $AUTH_ACCOUNT --allow-pubsub ">"
AUTH_USER_PUBKEY=$(nsc describe user --name $ORCHESTRATOR_AUTH_USER --account $AUTH_ACCOUNT --field sub | jq -r)
echo "assigned auth user pubkey: $AUTH_USER_PUBKEY"

# Step 7: Create "Sentinel" User in AUTH Account (used by host agents in auth-callout service)
nsc add user --name $AUTH_GUARD_USER --account $AUTH_ACCOUNT --deny-pubsub ">"

# Step 8: Configure Auth Callout
echo $AUTH_ACCOUNT_PUBKEY
echo $AUTH_SK_ACCOUNT_PUBKEY
nsc edit authcallout --account $AUTH_ACCOUNT --allowed-account "\"$AUTH_ACCOUNT_PUBKEY\",\"$AUTH_SK_ACCOUNT_PUBKEY\"" --auth-user $AUTH_USER_PUBKEY

# Step 9: Generate JWT files
nsc generate creds --name $ORCHESTRATOR_AUTH_USER --account $AUTH_ACCOUNT > $LOCAL_CREDS_PATH/$ORCHESTRATOR_AUTH_USER.creds # --> local to hub exclusively
nsc describe operator --raw --output-file $SHARED_CREDS_PATH/$OPERATOR.jwt
nsc describe account --name SYS --raw --output-file $SHARED_CREDS_PATH/$SYS_ACCOUNT.jwt
nsc generate creds --name $AUTH_GUARD_USER --account $AUTH_ACCOUNT --output-file $SHARED_CREDS_PATH/$AUTH_GUARD_USER.creds

extract_signing_key ADMIN $ADMIN_SK
echo "extracted ADMIN signing key"

extract_signing_key AUTH $AUTH_SK_ACCOUNT_PUBKEY
echo "extracted AUTH signing key"

extract_signing_key AUTH_ROOT $AUTH_ACCOUNT_PUBKEY
echo "extracted AUTH root key"

# Step 10: Generate Resolver Config
nsc generate config --nats-resolver --sys-account $SYS_ACCOUNT --force --config-file $RESOLVER_FILE

echo "Setup complete. Shared JWTs and resolver file are in the $SHARED_CREDS_PATH/ directory. Private creds are in the $LOCAL_CREDS_PATH/ directory."
echo "!! Don't forget to start the NATS server and push the credentials to the server with 'nsc push -A' !!"

#!/usr/bin/env bash
# shellcheck disable=SC2005,SC2086

# --------
# NB: This setup expects the `nats` and the `nsc` binarys to be locally installed and accessible.  This script will verify that they both exist locally before running setup commnds.

# Script Overview:
# This script is responsible for setting up the "Operator Chain of Trust" (eg: O/A/U) authentication pattern that is associated with the Orchestrator Hub on the Hosting Agent.

# Input Vars:
#   - SHARED_CREDS_DIR
#   - OPERATOR_JWT_PATH
#   - SYS_ACCOUNT_JWT_PATH
#   - AUTH_ACCOUNT_JWT_PATH

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
SYS_ACCOUNT_NAME="SYS"
AUTH_ACCOUNT_NAME="AUTH"
SHARED_CREDS_DIR="shared_creds_output"
OPERATOR_JWT_PATH="$SHARED_CREDS_DIR/$OPERATOR_NAME.jwt"
SYS_ACCOUNT_JWT_PATH="$SHARED_CREDS_DIR/$SYS_ACCOUNT_NAME.jwt"
AUTH_GUARD_USER_NAME="auth-guard"
AUTH_GUARD_USER_PATH="$SHARED_CREDS_DIR/$AUTH_GUARD_USER_NAME.creds"

if [ ! -d "$SHARED_CREDS_DIR" ]; then
    echo "Shared output dir not found. Unable to set up local chain of trust."
    exit 1
else
    if [ ! -d "$OPERATOR_JWT_PATH" ]; then
        echo "Operator JWT not found. Unable to set up local chain of trust."
        exit 1
    else
        echo "Found the $OPERATOR_JWT_PATH. Adding Operator to local chain reference."
        # Add Operator
        nsc add operator -u $OPERATOR_JWT_PATH --force
        echo "Operator added to local nsc successfully."

        if [ ! -d "$SYS_ACCOUNT_JWT_PATH" ]; then
            echo "SYS account JWT not found. Unable to add SYS ACCOUNT to the local chain of trust."
            exit 1
        else
            echo "Found the $SYS_ACCOUNT_JWT_PATH. Adding SYS Account to local chain reference."
            # Add SYS Account
            nsc import account --file $SYS_ACCOUNT_JWT_PATH
            echo "SYS account added to local nsc successfully."

            # TODO: For if/when add local sys user (that's) associated the Orchestrator SYS Account
            # if [ ! -d "$SYS_USER_PATH" ]; then
            #     echo "WARNING: SYS user JWT not found. Unable to add the SYS user as a locally trusted user."
            # else
            #     echo "Found the $SYS_USER_PATH usr to local chain reference."
            #     # Add SYS user
            #     nsc import user --file $SYS_USER_PATH
            #     # Create SYS user cred file and add to shared creds dir
            #     nsc generate creds --name $SYS_USER_NAME --account $SYS_ACCOUNT > $SHARED_CREDS_DIR/$SYS_USER_NAME.creds
            #     echo "SYS user added to local nsc successfully."
            # fi
        fi

        if [ ! -d "$AUTH_GUARD_USER_PATH" ]; then
            echo "WARNING: AUTH_GUARD user credentials not found. Unable to add the complete Hosting Agent set-up."
        else
            echo "Found the $AUTH_GUARD_USER_NAME credentials file."
            echo "Set-up complete. Credential files are in the $SHARED_CREDS_DIR/ directory."
        fi
    fi
fi


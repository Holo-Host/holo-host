#!/usr/bin/env bash
# shellcheck disable=SC2005,SC2086

# --------
# NB: This setup expects the `nats` and the `nsc` binarys to be locally installed and accessible.  This script will verify that they both exist locally before running setup commnds.

# Script Overview:
# This script is responsible for setting up the "Operator Chain of Trust" (eg: O/A/U) authentication pattern that is associated with the Orchestrator Hub on the Hosting Agent.

# Input Vars:
#   - SHARED_CREDS_PATH
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
NSC_PATH=$1
SHARED_CREDS_PATH=$2 # "shared_creds"
OPERATOR_NAME="HOLO"
SYS_ACCOUNT_NAME="SYS"
AUTH_ACCOUNT_NAME="AUTH"
OPERATOR_JWT_PATH="$SHARED_CREDS_PATH/$OPERATOR_NAME.jwt"
SYS_ACCOUNT_JWT_PATH="$SHARED_CREDS_PATH/$SYS_ACCOUNT_NAME.jwt"
AUTH_GUARD_USER_NAME="auth-guard"
AUTH_GUARD_USER_PATH="$SHARED_CREDS_PATH/$AUTH_GUARD_USER_NAME.creds"

if [ -d "$SHARED_CREDS_PATH" ]; then
    if [ -d "$OPERATOR_JWT_PATH" ]; then
        echo "Found the $OPERATOR_JWT_PATH."
        if nsc describe operator > /dev/null 2>&1; then
            echo "Operator already added to local nsc."
        else
            echo "Adding Operator to local chain reference."
            # Add Operator
            nsc add operator -u $OPERATOR_JWT_PATH --force
            echo "Operator added to local nsc successfully."
        fi
 
        if [ -d "$SYS_ACCOUNT_JWT_PATH" ]; then
            echo "Found the $SYS_ACCOUNT_JWT_PATH."
            if nsc describe account $SYS_ACCOUNT > /dev/null 2>&1; then
                echo "Operator already added to local nsc."
            else
                echo "Found the $SYS_ACCOUNT_JWT_PATH. Adding SYS Account to local chain reference."
                # Add SYS Account
                nsc import account --file $SYS_ACCOUNT_JWT_PATH
                echo "SYS account added to local nsc successfully."
            fi
        else
            echo "SYS account JWT not found. Unable to add SYS ACCOUNT to the local chain of trust."
            exit 1
        fi

        if [ -d "$AUTH_GUARD_USER_PATH" ]; then
            # Setup Auth Guard User
            echo "Found the $AUTH_GUARD_USER_NAME credentials file."
            $AUTH_GUARD_CRED_DIR_PATH="{$NSC_PATH}/keys/creds/{$OPERATOR_NAME}/{$AUTH_ACCOUNT_NAME}/"
            if [[ -f "$AUTH_GUARD_CRED_DIR_PATH/$$AUTH_GUARD_USER_NAME.creds" ]]; then
                echo "The auth guard user credential file is in the $AUTH_GUARD_CRED_DIR_PATH directory."
            else
                echo "Moving $AUTH_GUARD_USER_NAME creds to the $AUTH_GUARD_CRED_DIR_PATH directory."
                mv $AUTH_GUARD_USER_PATH $AUTH_GUARD_CRED_DIR_PATH
                echo "Set-up complete. The auth guard credential file is in the $AUTH_GUARD_CRED_DIR_PATH/ directory."
            fi
        else
            echo "WARNING: AUTH_GUARD user credentials not found. Unable to add the complete Hosting Agent set-up."
        fi
    else
        echo "Operator JWT not found. Unable to set up local chain of trust."
        exit 1
    fi
else
    echo "Shared output dir not found. Unable to set up local chain of trust."
    exit 1
fi


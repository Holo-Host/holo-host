#!/usr/bin/env bash
# shellcheck disable=SC2005,SC2086

# --------
# NB: This setup expects the `nats` and the `nsc` binarys to be locally installed and accessible.  
# This script will verify that they both exist locally before running setup commands.

# Script Overview:
# This script is responsible for setting up the "Operator Chain of Trust" (eg: O/A/U) 
# authentication pattern that is associated with the Orchestrator Hub on the Hosting Agent.

# Input Vars:
#   $1 - NSC_PATH: Path to NSC configuration directory
#   $2 - SHARED_CREDS_PATH: Path to shared credentials directory

# --------

set -e # Exit on any error

# Validate input parameters
if [ $# -ne 2 ]; then
    echo "ERROR: This script requires exactly 2 arguments:"
    echo "Usage: $0 <NSC_PATH> <SHARED_CREDS_PATH>"
    exit 1
fi
# Variables
NSC_PATH="$1"
SHARED_CREDS_PATH="$2"

# Validate NSC_PATH
if [ ! -d "$NSC_PATH" ]; then
    echo "ERROR: NSC_PATH '$NSC_PATH' does not exist or is not a directory"
    exit 1
fi

# Check for required commands
for cmd in nsc nats; do
    echo "Checking for command: $cmd"
    if command -v "$cmd" &>/dev/null; then
        echo "Found $cmd: $($cmd --version | head -n1)"
    else
        echo "ERROR: Command '$cmd' not found. Unable to proceed with setup.."
        exit 1
    fi
done

# Constants
OPERATOR_NAME="HOLO"
SYS_ACCOUNT_NAME="SYS"
AUTH_ACCOUNT_NAME="AUTH"
AUTH_GUARD_USER_NAME="auth-guard"

# File paths
OPERATOR_JWT_PATH="$SHARED_CREDS_PATH/$OPERATOR_NAME.jwt"
SYS_ACCOUNT_JWT_PATH="$SHARED_CREDS_PATH/$SYS_ACCOUNT_NAME.jwt"
AUTH_GUARD_USER_PATH="$SHARED_CREDS_PATH/$AUTH_GUARD_USER_NAME.creds"
AUTH_GUARD_CRED_DIR_PATH="$NSC_PATH/keys/creds/$OPERATOR_NAME/$AUTH_ACCOUNT_NAME"

echo "Setting up NATS authentication for host agent..."
echo "NSC_PATH: $NSC_PATH"
echo "SHARED_CREDS_PATH: $SHARED_CREDS_PATH"

# Check if shared creds directory exists
if [ ! -d "$SHARED_CREDS_PATH" ]; then
    echo "ERROR: Shared credentials directory '$SHARED_CREDS_PATH' not found."
    echo "Please ensure the credentials have been distributed to this host."
    exit 1
fi

# Add Operator JWT
if [ -f "$OPERATOR_JWT_PATH" ]; then
    echo "Found Operator JWT at: $OPERATOR_JWT_PATH"
    if nsc describe operator > /dev/null 2>&1; then
        echo "Operator already added to local NSC"
    else
        echo "Adding Operator to local chain reference..."
        nsc add operator -u "$OPERATOR_JWT_PATH" --force
        echo "Operator added to local NSC successfully"
    fi
else
    echo "ERROR: Operator JWT not found at '$OPERATOR_JWT_PATH'"
    echo "Unable to set up local chain of trust."
    exit 1
fi

# Add SYS Account JWT
if [ -f "$SYS_ACCOUNT_JWT_PATH" ]; then
    echo "Found SYS Account JWT at: $SYS_ACCOUNT_JWT_PATH"
    if nsc describe account "$SYS_ACCOUNT_NAME" > /dev/null 2>&1; then
        echo "SYS Account already added to local NSC"
    else
        echo "Adding SYS Account to local chain reference..."
        nsc import account --file "$SYS_ACCOUNT_JWT_PATH"
        echo "SYS Account added to local NSC successfully"
    fi
else
    echo "ERROR: SYS Account JWT not found at '$SYS_ACCOUNT_JWT_PATH'"
    echo "Unable to add SYS Account to the local chain of trust."
    exit 1
fi

# Setup Auth Guard User credentials
if [ -f "$AUTH_GUARD_USER_PATH" ]; then
    echo "Found Auth Guard User credentials at: $AUTH_GUARD_USER_PATH"
    
    # Create the target directory if it doesn't exist
    mkdir -p "$AUTH_GUARD_CRED_DIR_PATH"
    
    if [ -f "$AUTH_GUARD_CRED_DIR_PATH/$AUTH_GUARD_USER_NAME.creds" ]; then
        echo "Auth Guard User credentials already in place at: $AUTH_GUARD_CRED_DIR_PATH/$AUTH_GUARD_USER_NAME.creds"
    else
        echo "Moving Auth Guard User credentials to: $AUTH_GUARD_CRED_DIR_PATH/"
        mv "$AUTH_GUARD_USER_PATH" "$AUTH_GUARD_CRED_DIR_PATH/$AUTH_GUARD_USER_NAME.creds"
        echo "Auth Guard User credentials moved successfully"
    fi
else
    echo "WARNING: Auth Guard User credentials not found at '$AUTH_GUARD_USER_PATH'"
    echo "The host agent setup may be incomplete without these credentials."
fi

echo "Host agent authentication setup completed successfully"

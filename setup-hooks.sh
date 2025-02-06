#!/bin/bash

# Define source and destination paths
SRC_FILE=".github/hooks/pre-commit"
DEST_FILE=".git/hooks/pre-commit"

# Ensure the .git/hooks directory exists
mkdir -p .git/hooks

# Copy the pre-commit hook
if [ -f "$SRC_FILE" ]; then
    cp "$SRC_FILE" "$DEST_FILE"
    chmod +x "$DEST_FILE"
    echo "✅ Pre-commit hook installed successfully!"
else
    echo "❌ Error: $SRC_FILE does not exist!"
    exit 1
fi

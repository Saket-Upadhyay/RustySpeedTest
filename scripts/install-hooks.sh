#!/usr/bin/env sh
# Install repository-managed Git hooks by setting core.hooksPath.
# Run this once after cloning the repository.

set -e

if [ ! -d ".githooks" ]; then
  echo "No .githooks directory found."
  exit 1
fi

# Configure git to use the repository's .githooks directory
git config core.hooksPath .githooks

# Ensure hooks are executable
chmod +x .githooks/* || true

echo "Installed hooks (core.hooksPath set to .githooks)."

echo "Run 'git config --local --get core.hooksPath' to verify."



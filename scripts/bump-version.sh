#!/usr/bin/env bash
set -euo pipefail

CURRENT_VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
echo "Current version: $CURRENT_VERSION"

case "${1:-patch}" in
  "patch")
    NEW_VERSION=$(echo $CURRENT_VERSION | awk -F. '{$NF = $NF + 1;} 1' | sed 's/ /./g')
    ;;
  "minor")
    NEW_VERSION=$(echo $CURRENT_VERSION | awk -F. '{$(NF-1) = $(NF-1) + 1; $NF = 0} 1' | sed 's/ /./g')
    ;;
  "major")
    NEW_VERSION=$(echo $CURRENT_VERSION | awk -F. '{$1 = $1 + 1; $2 = 0; $3 = 0} 1' | sed 's/ /./g')
    ;;
  *)
    NEW_VERSION="$1"
    ;;
esac

echo "New version: $NEW_VERSION"

# Fix for macOS: Use sed -i '' (empty string for no backup)
if [[ "$OSTYPE" == "darwin"* ]]; then
  sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
else
  sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
fi
cargo check --quiet

echo "Updated Cargo.toml: $(grep '^version = ' Cargo.toml)"
#!/bin/bash
# Ad-hoc sign CAMPP for macOS distribution
# Usage: ./scripts/sign-macos.sh path/to/CAMPP.app

APP_PATH="${1:-src-tauri/target/release/bundle/macos/CAMPP.app}"

if [ ! -d "$APP_PATH" ]; then
    echo "Error: App not found at $APP_PATH"
    exit 1
fi

echo "Signing $APP_PATH with ad-hoc certificate..."
codesign --force --deep --sign - "$APP_PATH"

# Remove quarantine attribute
xattr -cr "$APP_PATH"

echo "Done! App should now open without the 'damaged' error."

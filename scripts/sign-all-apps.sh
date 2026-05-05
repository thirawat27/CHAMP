#!/bin/bash
# Find and sign all .app bundles in the target directory
# Usage: ./scripts/sign-all-apps.sh

TARGET_DIR="src-tauri/target"

echo "Finding .app bundles in $TARGET_DIR..."

find "$TARGET_DIR" -name "*.app" -type d | while read -r app_path; do
    echo ""
    echo "========================================="
    echo "Signing: $app_path"
    echo "========================================="

    # Ad-hoc sign
    codesign --force --deep --sign - "$app_path" 2>&1 || echo "Warning: codesign failed for $app_path"

    # Remove quarantine attribute
    xattr -cr "$app_path" 2>&1 || echo "Warning: xattr failed for $app_path"

    # Verify signature
    codesign --verify --verbose "$app_path" 2>&1 || echo "Warning: verification failed for $app_path"

    echo "✓ Signed: $app_path"
done

echo ""
echo "Done! All .app bundles have been ad-hoc signed."

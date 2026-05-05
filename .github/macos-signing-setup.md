# macOS Code Signing Setup Guide

This guide explains how to set up GitHub Actions for building, signing, and notarizing the CAMPP macOS app.

## Workflow

The main **"Build and Release"** workflow builds for all platforms (macOS, Windows, Linux) in a single run. macOS builds are signed and notarized automatically.

## Required GitHub Secrets

You need to add the following secrets to your GitHub repository (Settings → Secrets and variables → Actions → New repository secret):

### 1. APPLE_CERTIFICATE_BASE64
Your Developer ID certificate in Base64 format.

**How to create:**
1. Open **Keychain Access** on macOS
2. Find your "Developer ID Application" certificate
3. Right-click → Export → Save as `.p12` file
4. Set a password (remember it!)
5. Convert to Base64:
   ```bash
   base64 -i YourCertificate.p12 | pbcopy
   ```
6. Paste the Base64 string as the secret value

### 2. APPLE_CERTIFICATE_PASSWORD
The password you set when exporting your `.p12` certificate.

### 3. APPLE_ID
Your Apple ID email address (used for notarization).

### 4. APPLE_APP_PASSWORD
An app-specific password for notarization.

**How to create:**
1. Go to [appleid.apple.com](https://appleid.apple.com)
2. Sign in → Security section
3. App-Specific Passwords → Generate
4. Label it "CAMPP Notarization" and save the password

### 5. APPLE_TEAM_ID
Your Apple Developer Team ID: **BDA8FDADR2**

---

## Creating Your Developer ID Certificate

If you don't have a certificate yet:

1. Go to [developer.apple.com/account/resources/certificates/list](https://developer.apple.com/account/resources/certificates/list)
2. Click **+** → **Developer ID Application**
3. Follow the instructions using a Mac
4. Download and double-click to install in Keychain Access
5. Export as `.p12` following the steps above

---

## Running the Workflow

### Option 1: Tagged Release
Push a version tag to trigger the workflow:
```bash
git tag v0.2.0
git push origin v0.2.0
```

### Option 2: Manual Trigger
1. Go to Actions tab in GitHub
2. Select "Build Signed macOS"
3. Click "Run workflow" → Select branch → Run

---

## Workflow Outputs

The workflow produces:
- **CAMPP.app** - Universal binary (Apple Silicon + Intel)
- **CAMPP-universal.dmg** - DMG installer
- Both are code-signed and notarized
- Assets are attached to the GitHub Release

---

## Troubleshooting

**"No Developer ID Application certificate found"**
- Make sure APPLE_CERTIFICATE_BASE64 is properly set
- Verify the certificate is "Developer ID Application", not "Apple Distribution"

**Notarization fails**
- Check APPLE_ID and APPLE_APP_PASSWORD are correct
- Ensure the app is signed before notarization (handled by workflow)

**"The software has been altered" when running**
- The app needs to be stapled (handled by workflow)
- Make sure Gatekeeper is enabled

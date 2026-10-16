#!/bin/bash
# Creates a self-signed code-signing certificate "QuickTranslate Dev" in the login keychain,
# so rebuilds keep a stable identity and macOS keeps the Accessibility permission.
set -euo pipefail
NAME="QuickTranslate Dev"
if security find-identity -v -p codesigning | grep -q "\"$NAME\""; then
    echo "✓ '$NAME' already exists"; exit 0
fi
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
cd "$TMP"
openssl req -x509 -newkey rsa:2048 -keyout key.pem -out cert.pem -days 3650 -nodes \
    -subj "/CN=$NAME" -addext "keyUsage=critical,digitalSignature" -addext "extendedKeyUsage=critical,codeSigning" 2>/dev/null
openssl pkcs12 -export -out cert.p12 -inkey key.pem -in cert.pem -passout pass:qt -name "$NAME" -legacy 2>/dev/null
security import cert.p12 -k ~/Library/Keychains/login.keychain-db -P qt -T /usr/bin/codesign -T /usr/bin/security
security add-trusted-cert -r trustRoot -p codeSign -k ~/Library/Keychains/login.keychain-db cert.pem
echo "✓ created '$NAME' (remove it later via Keychain Access if you want)"

#!/usr/bin/env bash
set -euo pipefail

: "${MACOS_CERTIFICATE_P12:?MACOS_CERTIFICATE_P12 is required}"
: "${MACOS_CERTIFICATE_PASSWORD:?MACOS_CERTIFICATE_PASSWORD is required}"
: "${MACOS_KEYCHAIN_PASSWORD:?MACOS_KEYCHAIN_PASSWORD is required}"

certificate_path="$RUNNER_TEMP/portal-signing.p12"
keychain_path="$RUNNER_TEMP/portal-signing.keychain-db"

printf '%s' "$MACOS_CERTIFICATE_P12" | base64 --decode > "$certificate_path"
security create-keychain -p "$MACOS_KEYCHAIN_PASSWORD" "$keychain_path"
security set-keychain-settings -lut 21600 "$keychain_path"
security unlock-keychain -p "$MACOS_KEYCHAIN_PASSWORD" "$keychain_path"
security import "$certificate_path" \
  -P "$MACOS_CERTIFICATE_PASSWORD" \
  -A \
  -t cert \
  -f pkcs12 \
  -k "$keychain_path"
security list-keychain -d user -s "$keychain_path"
security set-key-partition-list \
  -S apple-tool:,apple:,codesign: \
  -s \
  -k "$MACOS_KEYCHAIN_PASSWORD" \
  "$keychain_path"
rm -f "$certificate_path"

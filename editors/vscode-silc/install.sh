#!/usr/bin/env bash
# Build the Silc language client + sil-lsp server, package a VSIX, and install it
# into Cursor (or VS Code).
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SRC/../.." && pwd)"
NAME="$(python3 -c "import json;print(json.load(open('$SRC/package.json'))['name'])")"
PUBLISHER="$(python3 -c "import json;print(json.load(open('$SRC/package.json'))['publisher'])")"
VERSION="$(python3 -c "import json;print(json.load(open('$SRC/package.json'))['version'])")"

CLI="${SILC_EDITOR_CLI:-}"
if [[ -z "$CLI" ]]; then
  for candidate in cursor code; do
    if command -v "$candidate" >/dev/null 2>&1; then CLI="$candidate"; break; fi
  done
fi
if [[ -z "$CLI" ]]; then
  echo "error: no 'cursor' or 'code' CLI on PATH." >&2
  echo "In Cursor run: Shell Command: Install 'cursor' command in PATH" >&2
  exit 1
fi

echo "==> Building sil-lsp (release)"
cargo build -p sil-lsp --release --manifest-path "$ROOT/Cargo.toml"

echo "==> Installing npm dependencies / compiling TypeScript"
if [[ ! -d "$SRC/node_modules" ]]; then
  (cd "$SRC" && npm install)
else
  (cd "$SRC" && npm install --no-audit --no-fund)
fi
(cd "$SRC" && npm run compile)

# Host triple for bundled server
OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS/$ARCH" in
  Darwin/arm64) TRIPLE="darwin-arm64" ;;
  Darwin/x86_64) TRIPLE="darwin-x64" ;;
  Linux/x86_64) TRIPLE="linux-x64" ;;
  Linux/aarch64|Linux/arm64) TRIPLE="linux-arm64" ;;
  MINGW*/x86_64|MSYS*/x86_64|CYGWIN*/x86_64) TRIPLE="win32-x64" ;;
  *) TRIPLE="$(echo "$OS" | tr '[:upper:]' '[:lower:]')-$ARCH" ;;
esac

SERVER_SRC="$ROOT/target/release/sil-lsp"
SERVER_NAME="sil-lsp-$TRIPLE"
if [[ "$TRIPLE" == win32-* ]]; then
  SERVER_SRC="${SERVER_SRC}.exe"
  SERVER_NAME="${SERVER_NAME}.exe"
fi
if [[ ! -f "$SERVER_SRC" ]]; then
  echo "error: missing built server at $SERVER_SRC" >&2
  exit 1
fi

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/extension/out" "$STAGE/extension/server" "$STAGE/extension/syntaxes"
cp "$SRC/package.json" "$SRC/language-configuration.json" "$STAGE/extension/"
cp -R "$SRC/syntaxes/." "$STAGE/extension/syntaxes/"
cp -R "$SRC/out/." "$STAGE/extension/out/"
[[ -f "$SRC/README.md" ]] && cp "$SRC/README.md" "$STAGE/extension/"
# Bundle the full production dependency tree (vscode-languageclient and everything
# it requires transitively). `npm ls` without --all only reports depth 0, which
# silently ships a broken extension that fails at require() time.
if [[ -d "$SRC/node_modules" ]]; then
  mkdir -p "$STAGE/extension/node_modules"
  while read -r dep; do
    rel="${dep#"$SRC/node_modules/"}"
    if [[ -n "$rel" && "$rel" != "$dep" && -e "$dep" ]]; then
      mkdir -p "$STAGE/extension/node_modules/$(dirname "$rel")"
      rm -rf "$STAGE/extension/node_modules/$rel"
      cp -R "$dep" "$STAGE/extension/node_modules/$rel"
    fi
  done < <(cd "$SRC" && npm ls --all --omit=dev --parseable 2>/dev/null)

  if [[ ! -d "$STAGE/extension/node_modules/vscode-languageclient" ]]; then
    echo "warning: dependency walk failed; copying full node_modules" >&2
    rm -rf "$STAGE/extension/node_modules"
    cp -R "$SRC/node_modules" "$STAGE/extension/node_modules"
  fi
fi

# Fail fast if the staged extension cannot actually load its entry point.
echo "==> Verifying packaged extension resolves its dependencies"
node -e '
const path = require("path");
const stage = process.argv[1];
const required = [
  "vscode-languageclient/node",
  "vscode-languageserver-protocol",
  "vscode-jsonrpc",
];
for (const mod of required) {
  try {
    require.resolve(mod, { paths: [path.join(stage, "node_modules")] });
  } catch (err) {
    console.error(`missing bundled dependency: ${mod}`);
    process.exit(1);
  }
}
' "$STAGE/extension"

cp "$SERVER_SRC" "$STAGE/extension/server/$SERVER_NAME"
chmod +x "$STAGE/extension/server/$SERVER_NAME"

cat >"$STAGE/[Content_Types].xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json"/>
  <Default Extension="vsixmanifest" ContentType="text/xml"/>
  <Default Extension="md" ContentType="text/markdown"/>
  <Default Extension="js" ContentType="application/javascript"/>
  <Default Extension="ts" ContentType="application/typescript"/>
  <Default Extension="map" ContentType="application/json"/>
  <Default Extension="" ContentType="application/octet-stream"/>
</Types>
XML

cat >"$STAGE/extension.vsixmanifest" <<XML
<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="${NAME}" Version="${VERSION}" Publisher="${PUBLISHER}"/>
    <DisplayName>Silc Language</DisplayName>
    <Description xml:space="preserve">Syntax highlighting and semantic hover for Silc (.silc) source files.</Description>
    <Categories>Programming Languages</Categories>
  </Metadata>
  <Installation>
    <InstallationTarget Id="Microsoft.VisualStudio.Code"/>
  </Installation>
  <Dependencies/>
  <Assets>
    <Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true"/>
  </Assets>
</PackageManifest>
XML

VSIX="$SRC/${NAME}-${VERSION}.vsix"
rm -f "$VSIX"
(cd "$STAGE" && zip -q -r "$VSIX" .)

"$CLI" --install-extension "$VSIX" --force
echo
echo "Installed ${PUBLISHER}.${NAME} ${VERSION} via '${CLI}'."
echo "Bundled language server: server/${SERVER_NAME}"
echo "Reload the window (Developer: Reload Window) to activate hover."
echo
echo "Dev tip: set silc.languageServerPath to:"
echo "  $SERVER_SRC"

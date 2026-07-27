#!/usr/bin/env bash
# Package the Silc grammar as a VSIX and install it into Cursor (or VS Code).
#
# The VSIX format is just a zip with `extension/`, `extension.vsixmanifest`,
# and `[Content_Types].xml` at the root, so no `vsce` toolchain is required.
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
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

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/extension"
cp "$SRC/package.json" "$SRC/language-configuration.json" "$STAGE/extension/"
cp -R "$SRC/syntaxes" "$STAGE/extension/"
[[ -f "$SRC/README.md" ]] && cp "$SRC/README.md" "$STAGE/extension/"

cat >"$STAGE/[Content_Types].xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json"/>
  <Default Extension="vsixmanifest" ContentType="text/xml"/>
  <Default Extension="md" ContentType="text/markdown"/>
</Types>
XML

cat >"$STAGE/extension.vsixmanifest" <<XML
<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="${NAME}" Version="${VERSION}" Publisher="${PUBLISHER}"/>
    <DisplayName>Silc Language</DisplayName>
    <Description xml:space="preserve">Syntax highlighting for Silc (.silc) source files.</Description>
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
echo "Reload the window (Developer: Reload Window) to activate the grammar."

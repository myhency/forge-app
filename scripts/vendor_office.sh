#!/usr/bin/env bash
# Copies the docx and pptx page-view libraries from npm into crates/forge-shared/assets/ (committed,
# so the build needs no network). Run it again only to change the versions.
set -euo pipefail
JSZIP=3.10.2
DOCX=0.4.1
PPTX=1.0.7
dest="$(cd "$(dirname "$0")/.." && pwd)/crates/forge-shared/assets"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cd "$work"
for spec in "jszip@$JSZIP" "docx-preview@$DOCX" "pptx-preview@$PPTX"; do
  npm pack "$spec" --silent >/dev/null
done
mkdir j d p
tar xzf jszip-"$JSZIP".tgz -C j
tar xzf docx-preview-"$DOCX".tgz -C d
tar xzf pptx-preview-"$PPTX".tgz -C p
cp j/package/dist/jszip.min.js "$dest/jszip.min.js"
cp d/package/dist/docx-preview.min.js "$dest/docx-preview.min.js"
cp p/package/dist/pptx-preview.umd.js "$dest/pptx-preview.umd.js"
echo "vendored jszip $JSZIP, docx-preview $DOCX and pptx-preview $PPTX into $dest (licences: assets/OFFICE-LICENSES)"

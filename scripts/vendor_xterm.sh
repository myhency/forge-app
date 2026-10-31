#!/usr/bin/env bash
# Copies xterm.js and its fit addon from npm into crates/forge-shared/assets/ (committed, so the
# build needs no network). Run it again only to change the versions.
set -euo pipefail
XTERM=5.5.0
FIT=0.10.0
dest="$(cd "$(dirname "$0")/.." && pwd)/crates/forge-shared/assets"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$dest"
cd "$work"
npm pack "@xterm/xterm@$XTERM" --silent
mkdir x && tar xzf xterm-xterm-"$XTERM".tgz -C x
cp x/package/lib/xterm.js "$dest/xterm.js"
cp x/package/css/xterm.css "$dest/xterm.css"
cp x/package/LICENSE "$dest/XTERM-LICENSE"
npm pack "@xterm/addon-fit@$FIT" --silent
mkdir f && tar xzf xterm-addon-fit-"$FIT".tgz -C f
cp f/package/lib/addon-fit.js "$dest/addon-fit.js"
echo "vendored xterm $XTERM and addon-fit $FIT into $dest"

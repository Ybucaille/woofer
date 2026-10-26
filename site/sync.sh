#!/bin/sh
# Copies the live-preview runtime into site/live/ so the site stays self-contained on GitHub Pages.
# Run it after changing a preset: node tools/build.mjs && site/sync.sh
set -e
cd "$(dirname "$0")/.."
cp web/shaders.generated.js site/live/shaders.generated.js
cp quickshell/analysis.js site/live/analysis.js
echo "site/live updated ($(grep -c '"id":' site/live/shaders.generated.js) presets)"

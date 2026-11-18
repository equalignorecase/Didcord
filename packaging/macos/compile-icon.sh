#!/bin/sh
# Stage the bundled icon; this repository does not include an Icon Composer source.
set -eu
resources="$1"
mkdir -p "$resources"
cp packaging/macos/Didcord.icns "$resources/Didcord.icns"

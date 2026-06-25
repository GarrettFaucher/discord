#!/usr/bin/env bash

set -euo pipefail

if [ $# -ne 3 ]; then
	echo "Usage: $0 <output_directory> <binary_name> <target_triple>"
	echo "Example: $0 ~/.config/opendeck/plugins/com.garrettfaucher.equibop.sdPlugin equibop x86_64-unknown-linux-gnu"
	exit 1
fi

# Clear stale PI build output so removed routes/components don't ship as orphan chunks.
rm -rf assets/pi

cd pi
deno task build
cd ..

rm -rf "$1"
cp -r assets/ "$1"

cargo build --release
cp target/release/$2 "$1/$2-$3"

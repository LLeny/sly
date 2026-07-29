#!/usr/bin/env bash
# Builds sly-launcher, installs it on an iOS simulator and runs it.
#
#   scripts/run-ios-sim.sh                     # build, boot, install, launch
#   scripts/run-ios-sim.sh --roms private      # also copy ROMs into the sandbox
#   scripts/run-ios-sim.sh --sim "iPad Pro 11-inch (M5)" --release
#   scripts/run-ios-sim.sh --logs              # follow the app's log afterwards
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ -z "${DEVELOPER_DIR:-}" && -d /Applications/Xcode.app ]]; then
	export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi

BUNDLE_ID="${SLY_IOS_BUNDLE_ID:-dev.layle.sly-launcher}"
device="${SLY_IOS_SIM:-iPhone 17 Pro}"
roms=""
logs=0
build_args=(--simulator)

while [[ $# -gt 0 ]]; do
	case "$1" in
	--sim)
		device="$2"
		shift
		;;
	--roms)
		roms="$2"
		shift
		;;
	--release) build_args+=(--release) ;;
	--logs) logs=1 ;;
	-h | --help)
		sed -n '1d; /^#/!q; s/^#[ ]\{0,1\}//p' "${BASH_SOURCE[0]}"
		exit 0
		;;
	*)
		echo "unknown argument: $1" >&2
		exit 1
		;;
	esac
	shift
done

if ! xcrun simctl list runtimes | grep -q iOS; then
	echo "no iOS simulator runtime installed; run: xcodebuild -downloadPlatform iOS" >&2
	exit 1
fi

"$ROOT/scripts/build-ios.sh" "${build_args[@]}"
app="target/ios/simulator/Sly Launcher.app"

echo "==> booting $device"
xcrun simctl boot "$device" 2>/dev/null || true
open -a Simulator
xcrun simctl bootstatus "$device" -b >/dev/null

echo "==> installing"
xcrun simctl terminate "$device" "$BUNDLE_ID" 2>/dev/null || true
xcrun simctl install "$device" "$app"

if [[ -n $roms ]]; then
	container="$(xcrun simctl get_app_container "$device" "$BUNDLE_ID" data)"
	mkdir -p "$container/Documents/roms"
	find "$roms" -type f \( -iname '*.sfc' -o -iname '*.smc' \) \
		-exec cp {} "$container/Documents/roms/" \;
	echo "==> copied $(ls "$container/Documents/roms" | wc -l | tr -d ' ') ROMs into the sandbox"
fi

echo "==> launching $BUNDLE_ID"
xcrun simctl launch "$device" "$BUNDLE_ID"

if [[ $logs == 1 ]]; then
	echo "==> streaming log (ctrl-c to stop)"
	xcrun simctl spawn "$device" log stream --style compact \
		--predicate 'process == "sly-launcher"'
else
	echo "    logs: xcrun simctl spawn \"$device\" log stream --style compact \\"
	echo "            --predicate 'process == \"sly-launcher\"'"
	echo "    screenshot: xcrun simctl io \"$device\" screenshot shot.png"
fi

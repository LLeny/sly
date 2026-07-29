#!/usr/bin/env bash
# Builds sly-launcher for iOS and wraps it in an app bundle.
#
#   scripts/build-ios.sh                     # simulator build, ad-hoc signed
#   scripts/build-ios.sh --device --install  # device build, installed over USB
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source "$ROOT/scripts/lib-ios.sh"

if [[ -z "${DEVELOPER_DIR:-}" && -d /Applications/Xcode.app ]]; then
	export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi

BUNDLE_ID="${SLY_IOS_BUNDLE_ID:-dev.layle.sly-launcher}"
IDENTITY="${SLY_IOS_IDENTITY:-}"
PROFILE="${SLY_IOS_PROFILE:-}"
DEVICE="${SLY_IOS_DEVICE:-}"
MIN_OS=16.0
PROFILE_DIRS=(
	"$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"
	"$HOME/Library/MobileDevice/Provisioning Profiles"
)

destination=simulator
profile=dev
install=0

while [[ $# -gt 0 ]]; do
	case "$1" in
	--device) destination=device ;;
	--simulator) destination=simulator ;;
	--release) profile=release ;;
	--install) install=1 ;;
	--bundle-id)
		BUNDLE_ID="$2"
		shift
		;;
	--identity)
		IDENTITY="$2"
		shift
		;;
	--profile)
		PROFILE="$2"
		shift
		;;
	--device-id)
		DEVICE="$2"
		shift
		;;
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

if [[ $destination == device ]]; then
	target=aarch64-apple-ios
	sdk_platform=iphoneos
	platform=iPhoneOS
else
	target=aarch64-apple-ios-sim
	sdk_platform=iphonesimulator
	platform=iPhoneSimulator
fi

build_args=(-p sly-launcher --target "$target")
profile_dir=debug
if [[ $profile == release ]]; then
	build_args+=(--release)
	profile_dir=release
fi

echo "==> cargo build ($target, $profile)"
IPHONEOS_DEPLOYMENT_TARGET="$MIN_OS" cargo build "${build_args[@]}"

binary="target/$target/$profile_dir/sly-launcher"
app="target/ios/$destination/Sly Launcher.app"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' frontends/sly-launcher/Cargo.toml | head -1)"
sdk_version="$(xcrun --sdk "$sdk_platform" --show-sdk-version)"

echo "==> bundling $app"
rm -rf "$app"
mkdir -p "$app"
cp "$binary" "$app/sly-launcher"
sed -e "s|@BUNDLE_ID@|$BUNDLE_ID|g" \
	-e "s|@VERSION@|$version|g" \
	-e "s|@PLATFORM@|$platform|g" \
	-e "s|@SDK_PLATFORM@|$sdk_platform|g" \
	-e "s|@SDK_VERSION@|$sdk_version|g" \
	-e "s|@MIN_OS@|$MIN_OS|g" \
	frontends/sly-launcher/ios/Info.plist >"$app/Info.plist"
plutil -lint "$app/Info.plist" >/dev/null

entitlements=""
log=""
cleanup() {
	[[ -z $entitlements ]] || rm -f "$entitlements"
	[[ -z $log ]] || rm -f "$log"
}
trap cleanup EXIT

if [[ $destination == device ]]; then
	if [[ -z $PROFILE ]]; then
		for dir in "${PROFILE_DIRS[@]}"; do
			[[ -d $dir ]] || continue
			while IFS= read -r candidate; do
				app_id="$(ios_profile_app_id "$candidate")"
				if [[ ${app_id#*.} == "$BUNDLE_ID" || ${app_id#*.} == "*" ]]; then
					PROFILE="$candidate"
					break
				fi
			done < <(ls -t "$dir"/*.mobileprovision 2>/dev/null)
			[[ -z $PROFILE ]] || break
		done
	fi
	if [[ -z $PROFILE ]]; then
		echo "!! no provisioning profile; run scripts/ios-provision.sh first." >&2
		exit 1
	fi

	cp "$PROFILE" "$app/embedded.mobileprovision"
	entitlements="$(mktemp -t sly-entitlements).plist"
	security cms -D -i "$PROFILE" |
		plutil -extract Entitlements xml1 -o "$entitlements" -
	profile_app_id="$(plutil -extract application-identifier raw -o - "$entitlements")"
	if [[ ${profile_app_id#*.} != "$BUNDLE_ID" && ${profile_app_id#*.} != "*" ]]; then
		echo "!! profile covers ${profile_app_id#*.}, not $BUNDLE_ID." >&2
		exit 1
	fi
	echo "==> profile $(basename "$PROFILE")"

	if [[ -z $IDENTITY ]]; then
		IDENTITY="$(security find-identity -v -p codesigning |
			sed -n 's/.*"\(Apple Development[^"]*\)".*/\1/p' | head -1)"
	fi
	if [[ -z $IDENTITY ]]; then
		echo "!! no Apple Development identity; run scripts/ios-provision.sh first." >&2
		exit 1
	fi
fi

echo "==> codesign (${IDENTITY:-ad-hoc})"
codesign_args=(--force --timestamp=none --sign "${IDENTITY:--}")
if [[ -n $entitlements ]]; then
	codesign_args+=(--entitlements "$entitlements")
fi
codesign "${codesign_args[@]}" "$app"

echo "==> built $app"

if [[ $destination == simulator ]]; then
	echo "    run it with: scripts/run-ios-sim.sh"
	exit 0
fi

if [[ $install == 0 ]]; then
	echo "    install with: xcrun devicectl device install app --device <udid> \"$app\""
	exit 0
fi

if [[ -z $DEVICE ]]; then
	if ! ios_find_device; then
		echo "!! no connected device found." >&2
		exit 1
	fi
	DEVICE="$ios_device_udid"
	echo "==> device $ios_device_name"
fi

log="$(mktemp -t sly-install)"
if ! xcrun devicectl device install app --device "$DEVICE" "$app" >"$log" 2>&1; then
	cat "$log" >&2
	exit 1
fi
echo "==> installed"

if ! xcrun devicectl device process launch --device "$DEVICE" "$BUNDLE_ID" >"$log" 2>&1; then
	if grep -q "explicitly trusted" "$log"; then
		echo "!! launch denied: trust the certificate on the device under" >&2
		echo "!! Settings > General > VPN & Device Management, then re-run." >&2
	else
		cat "$log" >&2
	fi
	exit 1
fi
echo "==> launched"

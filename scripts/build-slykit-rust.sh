#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

: "${PLATFORM_NAME:?This script must run from an Xcode build phase}"
: "${CONFIGURATION:?This script must run from an Xcode build phase}"
: "${DERIVED_FILE_DIR:?This script must run from an Xcode build phase}"

export PATH="${PATH}:${HOME}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin"

SLY_ARCH="${CURRENT_ARCH:-}"
if [[ -z $SLY_ARCH || $SLY_ARCH == undefined_arch ]]; then
	SLY_ARCH="${NATIVE_ARCH_ACTUAL:?Xcode did not provide a build architecture}"
fi

case "${PLATFORM_NAME}:${SLY_ARCH}" in
iphoneos:arm64) SLY_RUST_TARGET=aarch64-apple-ios ;;
iphonesimulator:arm64) SLY_RUST_TARGET=aarch64-apple-ios-sim ;;
iphonesimulator:x86_64) SLY_RUST_TARGET=x86_64-apple-ios ;;
macosx:arm64) SLY_RUST_TARGET=aarch64-apple-darwin ;;
macosx:x86_64) SLY_RUST_TARGET=x86_64-apple-darwin ;;
*)
	echo "Unsupported Rust target: ${PLATFORM_NAME}:${SLY_ARCH}" >&2
	exit 1
	;;
esac

build_args=(
	--manifest-path "$ROOT/Cargo.toml"
	--package sly-ffi
	--target "$SLY_RUST_TARGET"
)
SLY_RUST_PROFILE=debug
if [[ $CONFIGURATION == Release ]]; then
	build_args+=(--release)
	SLY_RUST_PROFILE=release
fi

cargo build "${build_args[@]}"

mkdir -p "$DERIVED_FILE_DIR/SlyRust"
cp \
	"$ROOT/target/$SLY_RUST_TARGET/$SLY_RUST_PROFILE/libsly.a" \
	"$DERIVED_FILE_DIR/SlyRust/libsly.a"

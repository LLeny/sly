ios_find_device() {
	ios_device_udid=""
	ios_device_name=""

	local json count i state
	json="$(mktemp -t sly-devices)"
	xcrun devicectl list devices --json-output "$json" >/dev/null 2>&1 || true
	count="$(plutil -extract result.devices raw -o - "$json" 2>/dev/null || echo 0)"
	for ((i = 0; i < count; i++)); do
		state="$(plutil -extract "result.devices.$i.connectionProperties.tunnelState" \
			raw -o - "$json" 2>/dev/null || true)"
		[[ $state == connected ]] || continue
		ios_device_udid="$(plutil -extract "result.devices.$i.hardwareProperties.udid" \
			raw -o - "$json" 2>/dev/null || true)"
		ios_device_name="$(plutil -extract "result.devices.$i.deviceProperties.name" \
			raw -o - "$json" 2>/dev/null || true)"
		[[ -n $ios_device_udid ]] && break
	done
	rm -f "$json"

	[[ -n $ios_device_udid ]]
}

ios_profile_app_id() {
	security cms -D -i "$1" 2>/dev/null |
		plutil -extract Entitlements.application-identifier raw -o - - 2>/dev/null || true
}

#!/usr/bin/env bash
# Gets Xcode to issue a development certificate and a provisioning profile for
# the launcher's bundle id, so scripts/build-ios.sh can sign a device build.
#
#   scripts/ios-provision.sh                 # use the team Xcode is signed in as
#   scripts/ios-provision.sh --team ABCDE12345
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
source "$ROOT/scripts/lib-ios.sh"

if [[ -z "${DEVELOPER_DIR:-}" && -d /Applications/Xcode.app ]]; then
	export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi

BUNDLE_ID="${SLY_IOS_BUNDLE_ID:-dev.layle.sly-launcher}"
TEAM="${SLY_IOS_TEAM:-}"
MIN_OS=16.0
PROFILE_DIR="$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"

while [[ $# -gt 0 ]]; do
	case "$1" in
	--bundle-id)
		BUNDLE_ID="$2"
		shift
		;;
	--team)
		TEAM="$2"
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

if [[ -z $TEAM ]]; then
	teams="$(mktemp -t sly-teams).plist"
	if defaults export com.apple.dt.Xcode - 2>/dev/null |
		plutil -extract IDEProvisioningTeams xml1 -o "$teams" - 2>/dev/null; then
		TEAM="$(python3 - "$teams" <<-'EOF' 2>/dev/null || true
			import plistlib, sys
			with open(sys.argv[1], "rb") as f:
			    teams = plistlib.load(f)
			for entries in teams.values():
			    for entry in entries:
			        if entry.get("teamID"):
			            print(entry["teamID"])
			            sys.exit(0)
		EOF
		)"
	fi
	rm -f "$teams"
fi

if [[ -z $TEAM ]]; then
	TEAM="$(security find-certificate -a -c "Apple Development" -p 2>/dev/null |
		openssl x509 -noout -subject 2>/dev/null |
		sed -n 's/.*OU[ ]*=[ ]*\([A-Z0-9]*\).*/\1/p' | head -1 || true)"
fi

if [[ -z $TEAM ]]; then
	echo "!! no development team found; create a certificate in Xcode >" >&2
	echo "!! Settings > Apple Accounts > Manage Certificates, or pass --team <id>." >&2
	exit 1
fi

if ! ios_find_device; then
	echo "!! no connected device; a profile can only be issued for a real one." >&2
	exit 1
fi

echo "==> team $TEAM, bundle $BUNDLE_ID, device $ios_device_name"

stub="$(mktemp -d -t sly-provision)"
trap 'rm -rf "$stub"' EXIT
mkdir -p "$stub/Provision.xcodeproj/xcshareddata/xcschemes"

echo 'int main(void) { return 0; }' >"$stub/main.m"

cat >"$stub/Provision.xcodeproj/project.pbxproj" <<EOF
// !\$*UTF8*\$!
{
	archiveVersion = 1;
	classes = {};
	objectVersion = 56;
	rootObject = AA00000000000000000001;
	objects = {
		AA00000000000000000001 = {
			isa = PBXProject;
			attributes = {LastUpgradeCheck = 1600;};
			buildConfigurationList = AA00000000000000000010;
			compatibilityVersion = "Xcode 14.0";
			developmentRegion = en;
			hasScannedForEncodings = 0;
			knownRegions = (en);
			mainGroup = AA00000000000000000020;
			productRefGroup = AA00000000000000000021;
			projectDirPath = "";
			projectRoot = "";
			targets = (AA00000000000000000030);
		};
		AA00000000000000000010 = {
			isa = XCConfigurationList;
			buildConfigurations = (AA00000000000000000011);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Debug;
		};
		AA00000000000000000011 = {
			isa = XCBuildConfiguration;
			buildSettings = {
				ALWAYS_SEARCH_USER_PATHS = NO;
				IPHONEOS_DEPLOYMENT_TARGET = $MIN_OS;
				SDKROOT = iphoneos;
			};
			name = Debug;
		};
		AA00000000000000000020 = {
			isa = PBXGroup;
			children = (AA00000000000000000022, AA00000000000000000021);
			sourceTree = "<group>";
		};
		AA00000000000000000021 = {
			isa = PBXGroup;
			children = (AA00000000000000000023);
			name = Products;
			sourceTree = "<group>";
		};
		AA00000000000000000022 = {
			isa = PBXFileReference;
			lastKnownFileType = sourcecode.c.objc;
			path = main.m;
			sourceTree = "<group>";
		};
		AA00000000000000000023 = {
			isa = PBXFileReference;
			explicitFileType = wrapper.application;
			includeInIndex = 0;
			path = Provision.app;
			sourceTree = BUILT_PRODUCTS_DIR;
		};
		AA00000000000000000024 = {
			isa = PBXBuildFile;
			fileRef = AA00000000000000000022;
		};
		AA00000000000000000030 = {
			isa = PBXNativeTarget;
			buildConfigurationList = AA00000000000000000031;
			buildPhases = (AA00000000000000000033);
			buildRules = ();
			dependencies = ();
			name = Provision;
			productName = Provision;
			productReference = AA00000000000000000023;
			productType = "com.apple.product-type.application";
		};
		AA00000000000000000031 = {
			isa = XCConfigurationList;
			buildConfigurations = (AA00000000000000000032);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Debug;
		};
		AA00000000000000000032 = {
			isa = XCBuildConfiguration;
			buildSettings = {
				CODE_SIGN_STYLE = Automatic;
				DEVELOPMENT_TEAM = $TEAM;
				GENERATE_INFOPLIST_FILE = YES;
				INFOPLIST_KEY_UIRequiresFullScreen = YES;
				INFOPLIST_KEY_UISupportedInterfaceOrientations = UIInterfaceOrientationPortrait;
				PRODUCT_BUNDLE_IDENTIFIER = "$BUNDLE_ID";
				PRODUCT_NAME = Provision;
				TARGETED_DEVICE_FAMILY = "1,2";
			};
			name = Debug;
		};
		AA00000000000000000033 = {
			isa = PBXSourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (AA00000000000000000024);
			runOnlyForDeploymentPostprocessing = 0;
		};
	};
}
EOF

cat >"$stub/Provision.xcodeproj/xcshareddata/xcschemes/Provision.xcscheme" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion="1600" version="1.7">
   <BuildAction parallelizeBuildables="YES" buildImplicitDependencies="YES">
      <BuildActionEntries>
         <BuildActionEntry buildForTesting="YES" buildForRunning="YES" buildForProfiling="YES" buildForArchiving="YES" buildForAnalyzing="YES">
            <BuildableReference BuildableIdentifier="primary" BlueprintIdentifier="AA00000000000000000030" BuildableName="Provision.app" BlueprintName="Provision" ReferencedContainer="container:Provision.xcodeproj"/>
         </BuildActionEntry>
      </BuildActionEntries>
   </BuildAction>
   <LaunchAction buildConfiguration="Debug"/>
</Scheme>
EOF

echo "==> asking Xcode to provision"
xcodebuild -quiet \
	-project "$stub/Provision.xcodeproj" \
	-scheme Provision \
	-destination "id=$ios_device_udid" \
	-configuration Debug \
	-allowProvisioningUpdates \
	-allowProvisioningDeviceRegistration \
	CONFIGURATION_BUILD_DIR="$stub/build" \
	build

profile=""
if [[ -d $PROFILE_DIR ]]; then
	while IFS= read -r candidate; do
		app_id="$(ios_profile_app_id "$candidate")"
		if [[ ${app_id#*.} == "$BUNDLE_ID" || ${app_id#*.} == "*" ]]; then
			profile="$candidate"
			break
		fi
	done < <(ls -t "$PROFILE_DIR"/*.mobileprovision 2>/dev/null)
fi

if [[ -z $profile ]]; then
	echo "!! Xcode reported success but left no profile for $BUNDLE_ID." >&2
	exit 1
fi

expiry="$(security cms -D -i "$profile" 2>/dev/null |
	plutil -extract ExpirationDate raw -o - - 2>/dev/null || true)"
echo "==> profile $(basename "$profile")${expiry:+, expires $expiry}"
echo "    now run: scripts/build-ios.sh --device --install"

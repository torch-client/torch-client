#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

export LC_ALL=C
export TZ=UTC

[ "${UNSIGNED:-0}" = 1 ] && RELEASE=1
RELEASE="${RELEASE:-0}"

pin_mismatch() {
  if [ "$RELEASE" = 1 ]; then
    echo "error: $1 (a release build needs the pinned version)" >&2
    exit 1
  fi
  echo "warning: $1; this APK will not match a release build" >&2
}

TARGET=aarch64-linux-android
ABI=arm64-v8a
PROFILE=android
MIN_SDK=28
TARGET_SDK=36
LIB=libtorch_client.so
APK="dist/$(python3 tools/artifact_name.py name android)"

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
[ -d "$SDK" ] || { echo "no Android SDK at $SDK (set ANDROID_HOME)" >&2; exit 1; }

NDK_VERSION=27.1.12297006
BUILD_TOOLS_VERSION=36.0.0
PLATFORM_VERSION=android-36
JDK_MAJOR=21

NDK="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [ -z "$NDK" ]; then
  if [ -d "$SDK/ndk/$NDK_VERSION" ]; then
    NDK="$SDK/ndk/$NDK_VERSION"
  else
    NDK=$(ls -d "$SDK"/ndk/*/ 2>/dev/null | sort -V | tail -1 || true)
  fi
fi
[ -n "$NDK" ] && [ -d "$NDK" ] || {
  echo "no NDK under $SDK/ndk (install $NDK_VERSION, or set ANDROID_NDK_HOME)" >&2
  exit 1
}
NDK="${NDK%/}"
NDK_FOUND=$(sed -n 's/^Pkg.Revision *= *//p' "$NDK/source.properties" 2>/dev/null | tr -d '[:space:]')
[ "$NDK_FOUND" = "$NDK_VERSION" ] || pin_mismatch "NDK at $NDK is ${NDK_FOUND:-unknown}, pinned $NDK_VERSION"
TOOLS="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"

BUILD_TOOLS="$SDK/build-tools/$BUILD_TOOLS_VERSION"
if [ ! -d "$BUILD_TOOLS" ]; then
  pin_mismatch "no build-tools $BUILD_TOOLS_VERSION under $SDK/build-tools"
  BUILD_TOOLS=$(ls -d "$SDK"/build-tools/*/ 2>/dev/null | sort -V | tail -1 || true)
  [ -n "$BUILD_TOOLS" ] || { echo "no build-tools under $SDK/build-tools" >&2; exit 1; }
fi
BUILD_TOOLS="${BUILD_TOOLS%/}"

PLATFORM="$SDK/platforms/$PLATFORM_VERSION"
if [ ! -d "$PLATFORM" ]; then
  pin_mismatch "no $PLATFORM_VERSION under $SDK/platforms"
  PLATFORM=$(ls -d "$SDK"/platforms/android-*/ 2>/dev/null | sort -V | tail -1 || true)
  [ -n "$PLATFORM" ] || { echo "no platform under $SDK/platforms" >&2; exit 1; }
fi
PLATFORM="${PLATFORM%/}"

PLATFORM_API=$(basename "$PLATFORM" | sed 's/^android-//; s/\..*//')
if [[ "$PLATFORM_API" =~ ^[0-9]+$ ]] && [ "$PLATFORM_API" -lt 34 ]; then
  echo "platform $PLATFORM is API $PLATFORM_API; the manifest needs 34 or newer" >&2
  exit 1
fi

JAVAC=javac
[ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/javac" ] && JAVAC="$JAVA_HOME/bin/javac"
command -v "$JAVAC" >/dev/null || {
  echo "no javac on PATH; install a JDK ($JDK_MAJOR) for the connection service" >&2
  exit 1
}
JDK_FOUND=$("$JAVAC" -version 2>&1 | sed -n 's/^javac \([0-9]*\).*/\1/p')
[ "$JDK_FOUND" = "$JDK_MAJOR" ] || pin_mismatch "javac is ${JDK_FOUND:-unknown}, pinned $JDK_MAJOR (set JAVA_HOME)"

echo "==> ndk         $NDK ($NDK_FOUND)"
echo "==> build-tools $BUILD_TOOLS"
echo "==> platform    $PLATFORM"
echo "==> javac       $JAVAC ($JDK_FOUND)"
[ "$RELEASE" = 1 ] && echo "==> release build (not debuggable)"

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TOOLS/aarch64-linux-android$MIN_SDK-clang"
export CC_aarch64_linux_android="$TOOLS/aarch64-linux-android$MIN_SDK-clang"
export CXX_aarch64_linux_android="$TOOLS/aarch64-linux-android$MIN_SDK-clang++"
export AR_aarch64_linux_android="$TOOLS/llvm-ar"
export RANLIB_aarch64_linux_android="$TOOLS/llvm-ranlib"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-Clink-arg=-Wl,-z,max-page-size=16384"

export CARGO_PROFILE_ANDROID_TRIM_PATHS=all
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
C_PREFIX_MAP="-ffile-prefix-map=$PWD=. -ffile-prefix-map=$CARGO_HOME_DIR=/cargo -ffile-prefix-map=$NDK=/ndk"
export CFLAGS_aarch64_linux_android="$C_PREFIX_MAP"
export CXXFLAGS_aarch64_linux_android="$C_PREFIX_MAP"

cargo_extra=()
[ "$RELEASE" = 1 ] && cargo_extra+=(--locked)
echo "==> cargo -Z trim-paths rustc --lib --crate-type cdylib --profile $PROFILE --target $TARGET --features mobile_ui ${cargo_extra[*]} $*"
cargo -Z trim-paths rustc \
  --lib --crate-type cdylib \
  --profile "$PROFILE" \
  --target "$TARGET" \
  --features mobile_ui \
  ${cargo_extra[@]+"${cargo_extra[@]}"} \
  "$@"

SO="target/$TARGET/$PROFILE/$LIB"
[ -f "$SO" ] || { echo "cargo produced no $SO" >&2; exit 1; }

STAGE=target/apk
rm -rf "$STAGE"
mkdir -p "$STAGE/lib/$ABI" "$STAGE/assets" dist

cp "$SO" "$STAGE/lib/$ABI/$LIB"

if "$TOOLS/llvm-readelf" -d "$SO" | grep -q "libc++_shared.so"; then
  echo "==> libc++_shared.so (the library links it)"
  cp "$NDK/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/$TARGET/libc++_shared.so" \
     "$STAGE/lib/$ABI/"
fi

if [ "${MC_BUNDLE_ASSETS:-0}" = 1 ]; then
  echo "==> assets (bundled)"
  python3 tools/pack_assets.py --out "$STAGE/assets/assets.bin"
else
  echo "==> assets: not bundled; the client downloads them on first launch"
fi

if [ -n "${MC_ADDRESS:-}" ]; then
  echo "==> launch target $MC_ADDRESS"
  printf '%s\n%s\n' "$MC_ADDRESS" "${MC_USERNAME:-}" > "$STAGE/assets/launch.txt"
fi

if [ -n "${MC_ENV:-}" ]; then
  echo "==> env $MC_ENV"
  tr ' ' '\n' <<< "$MC_ENV" | grep -v '^$' > "$STAGE/assets/env.txt"
fi

echo "==> javac + d8"
mkdir -p "$STAGE/classes"
"$JAVAC" -nowarn --release 11 \
  -classpath "$PLATFORM/android.jar" \
  -d "$STAGE/classes" \
  android/java/com/torchclient/game/*.java
mapfile -t class_files < <(find "$STAGE/classes" -name '*.class' | sort)
"$BUILD_TOOLS/d8" --release \
  --min-api "$MIN_SDK" \
  --lib "$PLATFORM/android.jar" \
  --output "$STAGE" \
  "${class_files[@]}"

echo "==> aapt2 compile"
"$BUILD_TOOLS/aapt2" compile --dir android/res -o "$STAGE/res.zip"

link_args=()
if [ "$RELEASE" != 1 ]; then
  link_args+=(--debug-mode)
fi

echo "==> aapt2 link"
"$BUILD_TOOLS/aapt2" link \
  -o "$STAGE/base.apk" \
  "$STAGE/res.zip" \
  -I "$PLATFORM/android.jar" \
  --manifest android/AndroidManifest.xml \
  --min-sdk-version "$MIN_SDK" \
  --target-sdk-version "$TARGET_SDK" \
  ${link_args[@]+"${link_args[@]}"}

echo "==> zip"
( cd "$STAGE"
  stored=(classes.dex)
  if [ -d assets ]; then mapfile -t -O 1 stored < <(find assets -type f | sort); fi
  mapfile -t deflated < <(find "lib/$ABI" -type f | sort)
  chmod 0644 "${stored[@]}" "${deflated[@]}"
  touch -h -d '1981-01-01 00:00:00' "${stored[@]}" "${deflated[@]}"
  zip -q -0 -X -D base.apk "${stored[@]}"
  zip -q -9 -X -D base.apk "${deflated[@]}" )

echo "==> zipalign"
"$BUILD_TOOLS/zipalign" -P 16 -f 4 "$STAGE/base.apk" "$STAGE/aligned.apk"

if [ "${UNSIGNED:-0}" = 1 ]; then
  cp "$STAGE/aligned.apk" dist/torch-client-unsigned.apk
  echo
  ls -lh dist/torch-client-unsigned.apk
  exit 0
fi

echo "==> apksigner"
KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"
[ -f "$KEYSTORE" ] || {
  echo "no debug keystore at $KEYSTORE; run any Android Studio build once, or" >&2
  echo "keytool -genkeypair -keystore $KEYSTORE -storepass android -keypass android \\" >&2
  echo "  -alias androiddebugkey -keyalg RSA -validity 10000 -dname 'CN=Android Debug'" >&2
  exit 1
}
KS_PASS_ARG=pass:android; [ -n "${ANDROID_KS_PASS:-}" ] && KS_PASS_ARG=env:ANDROID_KS_PASS
KEY_PASS_ARG=pass:android; [ -n "${ANDROID_KEY_PASS:-}" ] && KEY_PASS_ARG=env:ANDROID_KEY_PASS
"$BUILD_TOOLS/apksigner" sign \
  --alignment-preserved \
  --ks "$KEYSTORE" \
  --ks-pass "$KS_PASS_ARG" \
  --key-pass "$KEY_PASS_ARG" \
  --ks-key-alias "${ANDROID_KEY_ALIAS:-androiddebugkey}" \
  --out "$APK" \
  "$STAGE/aligned.apk"

echo
ls -lh "$APK"
sha256sum "$APK"
if [ "$RELEASE" = 1 ]; then
  "$BUILD_TOOLS/apksigner" verify --print-certs "$APK" | grep 'SHA-256' || true
fi
echo
echo "install it:  adb install -r $APK"
echo "watch it:    adb logcat -s torch-client:V RustStdoutStderr:V"

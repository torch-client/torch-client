#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

TARGET=aarch64-linux-android
ABI=arm64-v8a
PROFILE=android
MIN_SDK=28
TARGET_SDK=36
LIB=libtorch_client.so

SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
[ -d "$SDK" ] || { echo "no Android SDK at $SDK (set ANDROID_HOME)" >&2; exit 1; }

NDK="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [ -z "$NDK" ]; then
  NDK=$(ls -d "$SDK"/ndk/*/ 2>/dev/null | sort -V | tail -1 || true)
fi
[ -n "$NDK" ] && [ -d "$NDK" ] || {
  echo "no NDK under $SDK/ndk (install one in Android Studio, or set ANDROID_NDK_HOME)" >&2
  exit 1
}
TOOLS="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"

BUILD_TOOLS=$(ls -d "$SDK"/build-tools/*/ 2>/dev/null | sort -V | tail -1 || true)
[ -n "$BUILD_TOOLS" ] || { echo "no build-tools under $SDK/build-tools" >&2; exit 1; }

PLATFORM=$(ls -d "$SDK"/platforms/android-*/ 2>/dev/null | sort -V | tail -1 || true)
[ -n "$PLATFORM" ] || { echo "no platform under $SDK/platforms" >&2; exit 1; }

PLATFORM_API=$(basename "$PLATFORM" | sed 's/^android-//; s/\..*//')
if [[ "$PLATFORM_API" =~ ^[0-9]+$ ]] && [ "$PLATFORM_API" -lt 34 ]; then
  echo "platform $PLATFORM is API $PLATFORM_API; the manifest needs 34 or newer" >&2
  exit 1
fi

command -v javac >/dev/null || {
  echo "no javac on PATH; install a JDK (17 or newer) for the connection service" >&2
  exit 1
}

echo "==> ndk         $NDK"
echo "==> build-tools $BUILD_TOOLS"
echo "==> platform    $PLATFORM"

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TOOLS/aarch64-linux-android$MIN_SDK-clang"
export CC_aarch64_linux_android="$TOOLS/aarch64-linux-android$MIN_SDK-clang"
export CXX_aarch64_linux_android="$TOOLS/aarch64-linux-android$MIN_SDK-clang++"
export AR_aarch64_linux_android="$TOOLS/llvm-ar"
export RANLIB_aarch64_linux_android="$TOOLS/llvm-ranlib"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS="-Clink-arg=-Wl,-z,max-page-size=16384"

echo "==> cargo rustc --lib --crate-type cdylib --profile $PROFILE --target $TARGET $*"
cargo rustc \
  --lib --crate-type cdylib \
  --profile "$PROFILE" \
  --target "$TARGET" \
  --features mobile_ui \
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

# The connection service, the APK's one Java class. `--release 11` compiles
# against the JDK's own view of `java.*`, and android.jar supplies `android.*`;
# d8 then desugars the lambdas for the minimum API and writes classes.dex.
echo "==> javac + d8"
mkdir -p "$STAGE/classes"
javac -nowarn --release 11 \
  -classpath "$PLATFORM/android.jar" \
  -d "$STAGE/classes" \
  android/java/com/torchclient/game/*.java
"$BUILD_TOOLS/d8" --release \
  --min-api "$MIN_SDK" \
  --lib "$PLATFORM/android.jar" \
  --output "$STAGE" \
  $(find "$STAGE/classes" -name '*.class')

echo "==> aapt2 link"
"$BUILD_TOOLS/aapt2" link \
  -o "$STAGE/base.apk" \
  -I "$PLATFORM/android.jar" \
  --manifest android/AndroidManifest.xml \
  --min-sdk-version "$MIN_SDK" \
  --target-sdk-version "$TARGET_SDK" \
  --version-code 1 \
  --version-name "$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"

# Stored, not deflated, and both entries need it for different reasons. The
# manifest says `extractNativeLibs="false"`, so the loader maps the .so
# straight out of the APK and a compressed one cannot be mapped at all. The
# archive is read with `AAsset_getBuffer`, which maps an uncompressed entry and
# copies a compressed one; 12 MB is worth not copying at startup. The cost is
# an APK that is larger than it has to be, which `strip`/LTO in the `android`
# profile is the lever on.
echo "==> zip"
( cd "$STAGE" && zip -q -0 -X -r base.apk classes.dex "lib/$ABI" assets )

echo "==> zipalign"
# -P 16: 16 KB page alignment for the libraries, matching max-page-size above.
"$BUILD_TOOLS/zipalign" -P 16 -f 4 "$STAGE/base.apk" "$STAGE/aligned.apk"

echo "==> apksigner"
# The debug key Android Studio generates on first run, with the passwords the
# tooling has always used for it. It is a sideload key and nothing more: an APK
# signed with it cannot be published, and every machine's is different, so
# reinstalling over a build from elsewhere means uninstalling first.
KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/debug.keystore}"
[ -f "$KEYSTORE" ] || {
  echo "no debug keystore at $KEYSTORE; run any Android Studio build once, or" >&2
  echo "keytool -genkeypair -keystore $KEYSTORE -storepass android -keypass android \\" >&2
  echo "  -alias androiddebugkey -keyalg RSA -validity 10000 -dname 'CN=Android Debug'" >&2
  exit 1
}
"$BUILD_TOOLS/apksigner" sign \
  --ks "$KEYSTORE" \
  --ks-pass pass:android \
  --key-pass pass:android \
  --ks-key-alias androiddebugkey \
  --out dist/torch-client.apk \
  "$STAGE/aligned.apk"

echo
ls -lh dist/torch-client.apk
echo
echo "install it:  adb install -r dist/torch-client.apk"
echo "watch it:    adb logcat -s torch-client:V RustStdoutStderr:V"

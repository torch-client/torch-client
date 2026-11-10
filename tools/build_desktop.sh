#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

WINDOWS=0
kept=()
for arg in "$@"; do
  case "$arg" in
    --windows) WINDOWS=1 ;;
    *) kept+=("$arg") ;;
  esac
done
set -- ${kept[@]+"${kept[@]}"}

if (( WINDOWS )); then
  PLATFORM=windows
  TARGET=x86_64-pc-windows-gnu
  EXE=torch-client.exe
  STRIP=x86_64-w64-mingw32-strip
  BUILT="target/$TARGET/release/$EXE"
  target_args=(--target "$TARGET")
else
  if [[ "$(uname -m)" != x86_64 ]]; then
    echo "this machine is $(uname -m); the Linux zip is named for x86_64" >&2
    exit 1
  fi
  PLATFORM=linux
  EXE=torch-client
  STRIP=strip
  BUILT="target/release/$EXE"
  target_args=()
fi

for tool in zip "$STRIP"; do
  command -v "$tool" >/dev/null || { echo "no $tool on PATH" >&2; exit 1; }
done

NAME="$(python3 tools/artifact_name.py name "$PLATFORM" "$@")"

echo "==> cargo build --release --bin torch-client ${target_args[*]+"${target_args[*]}"} $*"
cargo build --release --bin torch-client ${target_args[@]+"${target_args[@]}"} "$@"
[ -f "$BUILT" ] || { echo "cargo produced no $BUILT" >&2; exit 1; }

STAGE="target/package/$PLATFORM"
rm -rf "$STAGE"
mkdir -p "$STAGE" dist
cp "$BUILT" "$STAGE/$EXE"
echo "==> $STRIP"
"$STRIP" "$STAGE/$EXE"

OUTPUT="$PWD/dist/$NAME"
rm -f "$OUTPUT"
echo "==> zip"
( cd "$STAGE" && zip -q -9 -X "$OUTPUT" "$EXE" )
rm -rf "$STAGE"

echo
ls -lh "dist/$NAME"
sha256sum "dist/$NAME"

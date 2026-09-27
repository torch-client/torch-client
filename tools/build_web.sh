#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

HARDEN=0
OPT=0
kept=()
for arg in "$@"; do
  case "$arg" in
    --harden) HARDEN=1 ;;
    --opt) OPT=1 ;;
    *) kept+=("$arg") ;;
  esac
done
set -- ${kept[@]+"${kept[@]}"}

TIMINGS=()
now_ms() {
  local t=${EPOCHREALTIME/[.,]/}
  echo $((t / 1000))
}
STEP_AT=$(now_ms)
mark() {
  local now elapsed
  now=$(now_ms)
  elapsed=$((now - STEP_AT))
  TIMINGS+=("$(printf '%4d.%03ds  %s' $((elapsed / 1000)) $((elapsed % 1000)) "$1")")
  STEP_AT=$now
}

OUT="${OUT:-web-output}"
if (( HARDEN )); then
  NAME="${NAME:-hardened}"
else
  NAME="${NAME:-default}"
fi
SITE="$OUT/$NAME/site"
SYMBOLS="$OUT/$NAME/symbols.txt"

DIST="$OUT/$NAME/.build"
STAGED_SYMBOLS="$OUT/$NAME/.symbols.txt"
rm -rf "$DIST"
mkdir -p "$DIST"
trap 'status=$?; if (( status )); then echo; echo "build failed; $SITE is untouched" >&2; fi' EXIT

TARGET=wasm32-unknown-unknown
PROFILE=web
WASM="target/$TARGET/$PROFILE/torch_client.wasm"

CARGO_DIR="${CARGO_HOME:-$HOME/.cargo}"
REMAP="--remap-path-prefix=$CARGO_DIR=/cargo --remap-path-prefix=$HOME=/home/user"
RUSTC_COMMIT="$(rustc -vV | sed -n 's/^commit-hash: //p')"
if [[ -n "$RUSTC_COMMIT" ]]; then
  REMAP="$REMAP --remap-path-prefix=/rustc/$RUSTC_COMMIT=/rust"
fi
export RUSTFLAGS="${RUSTFLAGS:-} $REMAP"

echo "==> cargo rustc --lib --crate-type cdylib --profile $PROFILE --target $TARGET $*"
cargo rustc --lib --crate-type cdylib --profile "$PROFILE" --target "$TARGET" "$@"
mark "cargo"

echo "==> wasm-bindgen"
wasm-bindgen --target web --out-dir "$DIST" --out-name torch-client --no-typescript "$WASM"
mark "wasm-bindgen"

defaults=on
features=""
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
  case "${args[i]}" in
    --no-default-features) defaults=off ;;
    --features|-F) features+=" ${args[i + 1]-}" ;;
    --features=*|-F=*) features+=" ${args[i]#*=}" ;;
  esac
done
if [[ "$defaults" == on || " ${features//,/ } " == *" asset_download "* ]]; then
  echo "==> assets: skipped, this build downloads its own"
else
  echo "==> assets"
  python3 tools/pack_assets.py --out "$DIST/assets.bin"
fi

WASM_OUT="$DIST/torch-client_bg.wasm"

WASM_OPT="${WASM_OPT:-wasm-opt}"
if (( ! OPT )); then
  echo "==> wasm-opt: skipped, pass --opt to run it"
elif ! command -v "$WASM_OPT" >/dev/null 2>&1; then
  echo "==> wasm-opt not found; skipping (install binaryen to run it)"
else
  echo "==> $("$WASM_OPT" --version) -O3 -g, from $(command -v "$WASM_OPT")"
  if "$WASM_OPT" -O3 -g "$WASM_OUT" -o "$WASM_OUT.opt"; then
    mv "$WASM_OUT.opt" "$WASM_OUT"
    python3 tools/wasm_repair.py "$WASM_OUT"
  else
    rm -f "$WASM_OUT.opt"
    echo "    wasm-opt failed; shipping the module as cargo left it"
  fi
fi
mark "wasm-opt"

echo "==> shaders"
python3 tools/wgsl_strip.py "$WASM_OUT"
mark "shaders"

echo "==> symbols"
python3 tools/wasm_symbols.py dump "$WASM_OUT" "$STAGED_SYMBOLS"
python3 tools/wasm_symbols.py strip "$WASM_OUT"
mark "symbols + strip"

if (( HARDEN )); then
  echo "==> scrub"
  python3 tools/wasm_scrub.py "$WASM_OUT"
else
  echo "==> scrub: skipped, pass --harden for a build to publish"
fi
mark "scrub"

echo "==> verify"
python3 tools/wasm_verify.py "$WASM_OUT"
mark "verify"

echo "==> js"
python3 tools/js_strip.py "$DIST/torch-client.js" $(find "$DIST/snippets" -name '*.js' 2>/dev/null)
mark "js"

echo "==> gzip"
gzip -9 -f "$WASM_OUT"
for f in "$DIST/torch-client.js" "$DIST/assets.bin"; do
  if [[ -f "$f" ]]; then
    gzip -9 -k -f "$f"
  fi
done
mark "gzip"

echo "==> name"
MODULE_HASH="$(sha256sum "$WASM_OUT.gz" | cut -c1-12)"
MODULE="torch-client_bg.$MODULE_HASH.wasm.gz"
mv "$WASM_OUT.gz" "$DIST/$MODULE"
echo "    $MODULE"
mark "name"

echo "==> page"
python3 tools/minify_page.py web/index.html "$DIST/index.html"

sed -i "s/torch-client_bg\.wasm\.gz/$MODULE/" "$DIST/index.html"

if ! grep -q "$MODULE" "$DIST/index.html"; then
  echo "the page does not name $MODULE; the MODULE literal in web/index.html moved" >&2
  exit 1
fi

cp web/worker.js "$DIST/worker.js"

mark "page"

echo "==> headers"
cat > "$DIST/_headers" <<HEADERS
/$MODULE
  Cache-Control: public, max-age=31536000, immutable

/index.html
  Cache-Control: no-cache

/
  Cache-Control: no-cache
HEADERS
mark "headers"

rm -rf "$SITE.previous"
if [[ -d "$SITE" ]]; then
  mv "$SITE" "$SITE.previous"
fi
mv "$DIST" "$SITE"
mv "$STAGED_SYMBOLS" "$SYMBOLS"
rm -rf "$SITE.previous"
DIST="$SITE"

LIMIT=$((25 * 1024 * 1024))
oversize=()
while IFS= read -r -d '' f; do
  (( $(stat -c%s "$f") > LIMIT )) && oversize+=("$f")
done < <(find "$DIST" -type f -print0)
if (( ${#oversize[@]} )); then
  echo
  echo "WARNING: over the 25 MiB per-file limit, this will not deploy to Pages:"
  for f in "${oversize[@]}"; do
    echo "  $(du -h "$f" | cut -f1)  $f"
  done
fi

echo
du -h "$DIST"/* | sort -k2
echo
echo "time:"
printf '  %s\n' "${TIMINGS[@]}"
echo
echo "symbols:   $SYMBOLS   (keep it, do not upload it)"
echo "serve it:  python3 -m http.server 8080 --directory $DIST"
echo "deploy it: upload $DIST as the site root; index.html needs nothing else"
if (( ! HARDEN )); then
  echo "publish:   rebuild with --harden first"
fi

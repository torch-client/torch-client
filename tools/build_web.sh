#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

HARDEN=0
OPT=0
CHROMEBOOK=0
APPLE=0
kept=()
for arg in "$@"; do
  case "$arg" in
    --harden) HARDEN=1 ;;
    --opt) OPT=1 ;;
    --chromebook) CHROMEBOOK=1 ;;
    --apple) APPLE=1 ;;
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
RELEASE_NAME="$OUT/$NAME/release-name.txt"
RELEASE="$(python3 tools/artifact_name.py name web "$@")"

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

SEED=0
if python3 tools/artifact_name.py has seed_cracker "$@"; then
  SEED=1
  SEED_LIB=$(cargo metadata --format-version 1 --all-features | python3 -c '
import json, os, sys
pkgs = [p for p in json.load(sys.stdin)["packages"] if p["name"] == "seedcrack"]
if len(pkgs) != 1:
    sys.exit("expected one seedcrack package in cargo metadata, found %d" % len(pkgs))
print(os.path.dirname(pkgs[0]["manifest_path"]))
')
  SEED_CRATE="$SEED_LIB/seedcrack-wasm"
  [ -f "$SEED_CRATE/Cargo.toml" ] || { echo "no seedcrack-wasm in $SEED_LIB" >&2; exit 1; }
  SEED_TARGET="$PWD/target/seedcrack-wasm"
  echo "==> seedcrack-wasm ($SEED_CRATE)"
  cargo build --release --target "$TARGET" --manifest-path "$SEED_CRATE/Cargo.toml" --target-dir "$SEED_TARGET"
  SEED_WASM_OUT="$DIST/seedcrack_wasm.wasm"
  cp "$SEED_TARGET/$TARGET/release/seedcrack_wasm.wasm" "$SEED_WASM_OUT"
  mark "seedcrack-wasm"
fi

echo "==> wasm-bindgen"
wasm-bindgen --target web --out-dir "$DIST" --out-name torch-client --no-typescript "$WASM"
mark "wasm-bindgen"

if python3 tools/artifact_name.py has asset_download "$@"; then
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
  if (( SEED )); then
    if "$WASM_OPT" -O3 "$SEED_WASM_OUT" -o "$SEED_WASM_OUT.opt"; then
      mv "$SEED_WASM_OUT.opt" "$SEED_WASM_OUT"
    else
      rm -f "$SEED_WASM_OUT.opt"
      echo "    wasm-opt failed on seedcrack-wasm; shipping it as cargo left it"
    fi
  fi
fi
mark "wasm-opt"

echo "==> shaders"
python3 tools/wgsl_strip.py "$WASM_OUT"
mark "shaders"

echo "==> symbols"
python3 tools/wasm_symbols.py dump "$WASM_OUT" "$STAGED_SYMBOLS"
python3 tools/wasm_symbols.py strip "$WASM_OUT"
if (( SEED )); then
  python3 tools/wasm_symbols.py strip "$SEED_WASM_OUT"
fi
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

GLUE_HASH="$(sha256sum "$DIST/torch-client.js" | cut -c1-12)"
GLUE="torch-client.$GLUE_HASH.js"
mv "$DIST/torch-client.js" "$DIST/$GLUE"
echo "    $GLUE"
mark "js"

echo "==> gzip"
gzip -9 -f "$WASM_OUT"
for f in "$DIST/$GLUE" "$DIST/assets.bin"; do
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
page_args=()
if (( CHROMEBOOK )); then
  page_args+=(--chromebook)
fi
if (( APPLE )); then
  page_args+=(--apple)
fi
python3 tools/minify_page.py ${page_args[@]+"${page_args[@]}"} web/index.html "$DIST/index.html"

sed -i "s/torch-client_bg\.wasm\.gz/$MODULE/" "$DIST/index.html"

if ! grep -q "$MODULE" "$DIST/index.html"; then
  echo "the page does not name $MODULE; the MODULE literal in web/index.html moved" >&2
  exit 1
fi

sed -i "s|\./torch-client\.js|./$GLUE|g" "$DIST/index.html"

if grep -q '\./torch-client\.js' "$DIST/index.html" || ! grep -q "$GLUE" "$DIST/index.html"; then
  echo "the page does not import $GLUE; the glue imports in web/index.html changed" >&2
  exit 1
fi

sed "s|\./torch-client\.js|./$GLUE|" web/worker.js > "$DIST/worker.js"

if ! grep -q "$GLUE" "$DIST/worker.js"; then
  echo "the worker does not import $GLUE; the glue import in web/worker.js changed" >&2
  exit 1
fi

WORKER="worker.$(sha256sum "$DIST/worker.js" | cut -c1-12).js"
mv "$DIST/worker.js" "$DIST/$WORKER"
sed -i "s|\./worker\.js|./$WORKER|g" "$DIST/index.html"

if grep -q '\./worker\.js' "$DIST/index.html" || ! grep -q "$WORKER" "$DIST/index.html"; then
  echo "the page does not name $WORKER; the torchWorker line in web/index.html changed" >&2
  exit 1
fi

if (( SEED )); then
  SEED_WASM="seedcrack_wasm.$(sha256sum "$SEED_WASM_OUT" | cut -c1-12).wasm"
  mv "$SEED_WASM_OUT" "$DIST/$SEED_WASM"
  SEED_WORKER="seed_worker.$(sha256sum web/seed_worker.js | cut -c1-12).js"
  cp web/seed_worker.js "$DIST/$SEED_WORKER"
  sed -i -e "s|\./seedcrack_wasm\.wasm|./$SEED_WASM|g" -e "s|\./seed_worker\.js|./$SEED_WORKER|g" "$DIST/index.html"
  if ! grep -q "$SEED_WASM" "$DIST/index.html" || ! grep -q "$SEED_WORKER" "$DIST/index.html"; then
    echo "the page does not name $SEED_WASM and $SEED_WORKER; the torchSeed lines in web/index.html changed" >&2
    exit 1
  fi
  echo "    $SEED_WORKER, $SEED_WASM"
fi

if (( CHROMEBOOK || APPLE )); then
  cp web/chromebook/manifest.webmanifest web/chromebook/icon-*.png "$DIST/"
fi
if (( CHROMEBOOK )); then
  cached=(index.html "$GLUE" "$MODULE" "$WORKER" manifest.webmanifest icon-192.png icon-512.png icon-maskable-512.png)
  if [[ -f "$DIST/assets.bin" ]]; then
    cached+=(assets.bin)
  fi
  if (( SEED )); then
    cached+=("$SEED_WORKER" "$SEED_WASM")
  fi
  while IFS= read -r f; do
    cached+=("${f#"$DIST/"}")
  done < <(find "$DIST/snippets" -name '*.js' 2>/dev/null | sort)
  cache_name="torch-$( (cd "$DIST" && cat "${cached[@]}") | sha256sum | cut -c1-12)"
  files_list=""
  for f in "${cached[@]}"; do
    [[ "$f" == index.html ]] && f=""
    files_list+="${files_list:+, }\"./$f\""
  done
  sed -e "s|\"@CACHE@\"|\"$cache_name\"|" -e "s|\"@FILES@\"|$files_list|" web/chromebook/sw.js > "$DIST/sw.js"
  if grep -q '@CACHE@\|@FILES@' "$DIST/sw.js"; then
    echo "a placeholder survived in sw.js; the CACHE or FILES line in web/chromebook/sw.js changed" >&2
    exit 1
  fi
  echo "    sw.js caches ${#cached[@]} files as $cache_name"
fi

mark "page"

echo "==> headers"
cat > "$DIST/_headers" <<HEADERS
/$MODULE
  Cache-Control: public, max-age=31536000, immutable

/$GLUE
  Cache-Control: public, max-age=31536000, immutable

/$WORKER
  Cache-Control: public, max-age=31536000, immutable

/index.html
  Cache-Control: no-cache

/snippets/*
  Cache-Control: no-cache

/
  Cache-Control: no-cache
HEADERS
if (( SEED )); then
  cat >> "$DIST/_headers" <<HEADERS

/$SEED_WORKER
  Cache-Control: public, max-age=31536000, immutable

/$SEED_WASM
  Cache-Control: public, max-age=31536000, immutable
HEADERS
fi
if (( CHROMEBOOK )); then
  cat >> "$DIST/_headers" <<'HEADERS'

/sw.js
  Cache-Control: no-cache
HEADERS
fi
if (( CHROMEBOOK || APPLE )); then
  cat >> "$DIST/_headers" <<'HEADERS'

/manifest.webmanifest
  Cache-Control: no-cache
HEADERS
fi
mark "headers"

rm -rf "$SITE.previous"
if [[ -d "$SITE" ]]; then
  mv "$SITE" "$SITE.previous"
fi
mv "$DIST" "$SITE"
mv "$STAGED_SYMBOLS" "$SYMBOLS"
echo "$RELEASE" > "$RELEASE_NAME"
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
echo "pack it:   python3 tools/pack_single_html.py $DIST   (writes dist/$RELEASE)"
if (( ! HARDEN )); then
  echo "publish:   rebuild with --harden first"
fi

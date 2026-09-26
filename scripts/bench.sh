#!/usr/bin/env bash
# Compares Secopy with cp and rsync on real hardware (RFD NFR-1..NFR-3, milestone M0).
#
# Usage: scripts/bench.sh [--generate] [--purge] <source-dir> <dest-dir>
#   --generate  first fill <source-dir> with the reference data sets:
#               large/ (LARGE_GIB x 1 GiB files, default 4)
#               small/ (SMALL_COUNT x 16 KiB files, default 20000)
#   --purge     drop the OS file cache before every run (asks for sudo), so the
#               source is read from the device and not from RAM
#
# Put <source-dir> and <dest-dir> on the devices you want to measure
# (e.g. card reader -> SSD). Needs hyperfine and python3.
# Results go to docs/benchmarks/<date>-<host>.md.
set -euo pipefail

generate=false
purge=false
while [[ $# -gt 0 && $1 == --* ]]; do
  case $1 in
    --generate) generate=true ;;
    --purge) purge=true ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
  shift
done
[[ $# -eq 2 ]] || { sed -n '4,15p' "$0"; exit 2; }
src=$1
dest=$2
command -v hyperfine >/dev/null || { echo "hyperfine is required (brew/apt install hyperfine)" >&2; exit 2; }

repo=$(cd "$(dirname "$0")/.." && pwd)
cargo build --release -p secopy-cli --manifest-path "$repo/Cargo.toml"
secopy="$repo/target/release/secopy-cli"

if $generate; then
  mkdir -p "$src"
  python3 - "$src" "${LARGE_GIB:-4}" "${SMALL_COUNT:-20000}" <<'PY'
import os, sys
root, large, small = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
os.makedirs(f"{root}/large", exist_ok=True)
chunk = os.urandom(1 << 20)
for i in range(large):
    with open(f"{root}/large/clip{i:02}.bin", "wb") as f:
        for _ in range(1024):
            f.write(chunk)
data = os.urandom(16 << 10)
for i in range(small):
    d = f"{root}/small/d{i % 100:03}"
    os.makedirs(d, exist_ok=True)
    with open(f"{d}/f{i:05}.bin", "wb") as f:
        f.write(data)
PY
fi

drop_caches=true
if $purge; then
  case $(uname) in
    Darwin) drop_caches="sync && sudo purge" ;;
    Linux) drop_caches="sync && echo 3 | sudo tee /proc/sys/vm/drop_caches >/dev/null" ;;
  esac
  sudo -v
fi

out_dir="$repo/docs/benchmarks"
mkdir -p "$out_dir"
report="$out_dir/$(date +%Y-%m-%d)-$(hostname -s).md"
{
  echo "# Benchmark $(date '+%Y-%m-%d %H:%M') on $(hostname -s)"
  echo
  echo "- OS: $(uname -sr)"
  echo "- Source: \`$src\`"
  echo "- Destination: \`$dest\`"
  echo "- Cache purged between runs: $purge"
  echo "- Secopy: $("$secopy" --version)"
} >"$report"

for set in large small; do
  [[ -d "$src/$set" ]] || continue
  run="$dest/secopy-bench-run"
  prepare="rm -rf '$run' && mkdir -p '$run' && $drop_caches"
  tmp=$(mktemp)
  hyperfine --runs 3 --prepare "$prepare" --export-markdown "$tmp" \
    -n "cp -R" "cp -R '$src/$set' '$run/'" \
    -n "rsync -a" "rsync -a '$src/$set' '$run/'" \
    -n "secopy copy" "'$secopy' '$src/$set' --to '$run'" \
    -n "secopy copy+verify" "'$secopy' '$src/$set' --to '$run' --verify"
  { echo; echo "## $set/ ($(du -sh "$src/$set" | cut -f1))"; echo; cat "$tmp"; } >>"$report"
  rm -rf "$tmp" "$run"
done
echo "Results: $report"

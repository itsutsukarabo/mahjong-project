#!/usr/bin/env bash
# CI（.github/workflows/ci.yml）と同じ検査を手元で回す。1つでも失敗したら非ゼロで終わる。
# pre-push フック（.githooks/pre-push）から呼ばれる。手で回すときは `bash tools/check.sh`。
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
step() { echo "== $*"; }
step "cargo fmt --check"
cargo fmt --all -- --check
step "cargo clippy"
cargo clippy -q --workspace --all-targets --locked -- -D warnings
step "cargo test"
log="$(mktemp)"
if ! cargo test -q --workspace --locked >"$log" 2>&1; then
  grep -E "panicked|FAILED|error" "$log" | head -40
  rm -f "$log"
  echo "TESTS FAILED"
  exit 1
fi
grep -E "^test result" "$log" | awk '{p+=$4; f+=$6} END {print "tests: " p " passed, " f " failed"}'
rm -f "$log"
step "rules catalog / coverage / questions"
cargo run --locked -q -p rules-catalog -- check
cargo run --locked -q -p rules-catalog -- coverage --strict
cargo run --locked -q -p rules-catalog -- questions-check rules/mjsoul-sanma.toml docs/spec/RULES_QUESTIONS_MJSOUL.md
step "log TS types (web/packages/log-types) are generated from mj-log"
cargo test --locked -q -p mj-log --features ts --test ts_types
step "scenarios list"
cargo run --locked -q -p mj-scenarios -- check
step "log samples"
PY=""
for c in python python3; do
  if "$c" -c "import sys" >/dev/null 2>&1; then PY="$c"; break; fi
done
[ -n "$PY" ] || { echo "python not found"; exit 1; }
"$PY" -I -X utf8 docs/spec/samples/make_sample_log.py --check
echo "ALL CHECKS OK"

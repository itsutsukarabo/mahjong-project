#!/bin/bash
# riichienv-core Rust-only bench runner (WSL, offline with vendored crates)
set -e
source ~/.cargo/env
rm -rf ~/rbench/bench/vendor ~/rbench/bench/.cargo
cp -r /mnt/c/rbench/bench/vendor ~/rbench/bench/vendor
cp /mnt/c/rbench/bench/Cargo.lock ~/rbench/bench/Cargo.lock
mkdir -p ~/rbench/bench/.cargo
cat > ~/rbench/bench/.cargo/config.toml <<'CFG'
[source.crates-io]
replace-with = "vendored-sources"
[source.vendored-sources]
directory = "vendor"
CFG
cd ~/rbench/bench && cargo build --release --offline 2>&1 | tail -3
B=./target/release/riichienv-core-bench
for a in "$@"; do $B $a; done

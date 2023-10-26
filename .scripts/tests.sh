set -euxo pipefail

export rt='rust-tools --template you-rust'

export CARGO_TARGET_DIR="$($rt target-dir)"
export RUSTFLAGS="$($rt rust-flags)"

$rt rustfmt
$rt clippy

$rt test-generic .

cargo test --bin cli-server --features="std,tokio/macros,tokio/rt-multi-thread"

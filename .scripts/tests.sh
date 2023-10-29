set -euxo pipefail

export rt='rust-tools --template you-rust'

export CARGO_TARGET_DIR="$($rt target-dir)"
export RUSTFLAGS="$($rt rust-flags)"

$rt rustfmt
$rt clippy

$rt test-generic .

cargo test --bin server --features="rustls-pemfile,tokio/macros,tokio/rt-multi-thread"

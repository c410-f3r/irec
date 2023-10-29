set -euxo pipefail

cargo install rust-tools --git https://github.com/c410-f3r/regular-crates

rt='rust-tools --template you-rust'

export CARGO_TARGET_DIR="$($rt target-dir)"
export RUSTFLAGS="$($rt rust-flags)"

$rt rustfmt
$rt clippy

$rt test-generic .

cargo test --bin server --features="rustls-pemfile,tokio/macros,tokio/rt-multi-thread"

#!/usr/bin/env bash
# cargo for this repo inside WSL, on a machine without a system C toolchain.
#
# Rust needs a C linker (`cc`) for native binaries and build scripts. With build-essential installed this is plain
# cargo. Otherwise, in order of preference:
#   1. zig as the C compiler: a `cc` wrapper in ~/.local/zig-cc that runs `zig cc -target x86_64-linux-gnu`.
#      Everything works, including wasmi's build scripts and `cargo test --workspace`.
#   2. musl + the lld that ships with Rust: enough for the compiler and the web build, but not for wasmi's build
#      scripts, so use `--no-default-features` for wahoo-cli.
set -euo pipefail
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if ! command -v cc >/dev/null 2>&1 && [ -x "$HOME/.local/zig-cc/cc" ]; then
  export PATH="$HOME/.local/zig-cc:$PATH"
fi
if ! command -v cc >/dev/null 2>&1; then
  export CARGO_BUILD_TARGET="${CARGO_BUILD_TARGET:-x86_64-unknown-linux-musl}"
  export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld
fi
exec cargo "$@"

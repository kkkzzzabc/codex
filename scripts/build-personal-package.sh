#!/usr/bin/env bash
# Build the personal macOS package without changing global development settings.
set -euo pipefail

# The package builder calls this same executable as its Cargo adapter.
if [[ "${1:-}" == "build" && -n "${CODEX_PERSONAL_REAL_CARGO:-}" ]]; then
  exec "$CODEX_PERSONAL_REAL_CARGO" "$@" --locked
fi

usage() {
  cat <<'EOF'
Usage: build-personal-package.sh --work-dir DIR --python FILE [options]

  --work-dir DIR     Prepared, isolated build environment outside the repository.
  --python FILE      Absolute path to Python 3.11 or newer.
  --rustup FILE      Absolute path to rustup (default: PATH or Homebrew).
  --package-dir DIR  New package output directory (default: WORK/packages/TIMESTAMP).
  --dry-run         Validate inputs and print the command without building or writing.
  --help            Show this help.

Requires macOS arm64 and WORK/tools/rustup with the repository's Rust toolchain.
See docs/fork-maintenance.md for preparation and cleanup instructions.
EOF
}

fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

work_dir=''
python_bin=''
rustup_bin=''
package_dir=''
dry_run=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --work-dir|--python|--rustup|--package-dir)
      [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || fail "Missing value for $1"
      case "$1" in
        --work-dir) work_dir=$2 ;;
        --python) python_bin=$2 ;;
        --rustup) rustup_bin=$2 ;;
        --package-dir) package_dir=$2 ;;
      esac
      shift 2
      ;;
    --dry-run) dry_run=true; shift ;;
    --help) usage; exit 0 ;;
    *) fail "Unknown argument: $1" ;;
  esac
done

[[ -n "$work_dir" && -n "$python_bin" ]] || { usage >&2; exit 1; }
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || fail 'Only macOS arm64 is supported.'
[[ "$work_dir" == /* && -d "$work_dir" ]] || fail 'Use an absolute, existing work directory; prepare tools first.'
[[ "$python_bin" == /* && -x "$python_bin" ]] || fail 'Use an absolute executable Python path; prepare Python first.'

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(cd "$script_dir/.." && pwd -P)
script="$script_dir/$(basename "${BASH_SOURCE[0]}")"
work_dir=$(cd "$work_dir" && pwd -P)
case "$work_dir/" in
  "$repo_root/"*) fail 'Keep the build environment outside the repository.' ;;
esac

if [[ -z "$rustup_bin" ]]; then
  rustup_bin=$(command -v rustup || true)
  if [[ -z "$rustup_bin" && -x /opt/homebrew/opt/rustup/bin/rustup ]]; then
    rustup_bin=/opt/homebrew/opt/rustup/bin/rustup
  fi
fi
[[ "$rustup_bin" == /* && -x "$rustup_bin" ]] || fail 'rustup is missing; prepare it using the environment setup instructions.'
rustup_bin=$(PYTHONDONTWRITEBYTECODE=1 "$python_bin" -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve())' "$rustup_bin")
rustup_dir=$(cd "$(dirname "$rustup_bin")" && pwd -P)
[[ -x "$rustup_dir/cargo" ]] || fail 'Cargo proxy was not found beside rustup.'

toolchain=$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' "$repo_root/codex-rs/rust-toolchain.toml")
[[ -n "$toolchain" && "$toolchain" != *$'\n'* ]] || fail 'Cannot read the repository Rust toolchain.'
[[ -x "$work_dir/tools/rustup/toolchains/$toolchain-aarch64-apple-darwin/bin/rustc" ]] || fail "Rust $toolchain is not prepared in WORK/tools/rustup."
PYTHONDONTWRITEBYTECODE=1 "$python_bin" -c 'import sys; assert sys.version_info >= (3, 11), "Python 3.11+ required"; import tomllib' || fail 'Python 3.11+ with tomllib is required.'

if [[ -z "$package_dir" ]]; then
  package_dir="$work_dir/packages/$(date -u +%Y%m%dT%H%M%SZ)"
fi
[[ "$package_dir" == /* ]] || fail 'Use an absolute package output path.'
package_dir=$(PYTHONDONTWRITEBYTECODE=1 "$python_bin" -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve())' "$package_dir")
case "$package_dir/" in
  "$work_dir/packages/"*) ;;
  *) fail 'Keep new packages under WORK/packages/.' ;;
esac
[[ ! -e "$package_dir" && ! -L "$package_dir" ]] || fail 'Package output already exists; choose a new path. Existing packages are never overwritten.'

build_env=(
  -u V8_FROM_SOURCE -u RUSTY_V8_ARCHIVE -u RUSTY_V8_SRC_BINDING_PATH
  -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS
  "PATH=$rustup_dir:$PATH"
  "RUSTUP_HOME=$work_dir/tools/rustup"
  "RUSTUP_TOOLCHAIN=$toolchain"
  'RUSTUP_AUTO_INSTALL=0'
  "CARGO_HOME=$work_dir/deps/cargo"
  "CARGO_TARGET_DIR=$work_dir/target"
  'CARGO_INCREMENTAL=0' 'CARGO_BUILD_JOBS=2'
  "CODEX_REPO_ROOT=$repo_root"
  "CODEX_PERSONAL_REAL_CARGO=$rustup_dir/cargo"
  "TMPDIR=$work_dir/tmp"
  'PYTHONDONTWRITEBYTECODE=1' 'PYTHONUNBUFFERED=1'
)
build_command=(
  "$python_bin" "$script_dir/build_codex_package.py"
  --target aarch64-apple-darwin --variant codex --cargo-profile dev-small
  --cargo "$script" --package-dir "$package_dir"
)
printf 'env'
printf ' %q' "${build_env[@]}" "${build_command[@]}"
printf '\n'
if "$dry_run"; then
  printf 'Dry run only: no files created, downloads, installation, or compilation.\n'
  exit 0
fi

mkdir -p "$work_dir/tmp" "$work_dir/deps" "$work_dir/target" "$(dirname "$package_dir")"
cd "$repo_root"
exec env "${build_env[@]}" "${build_command[@]}"

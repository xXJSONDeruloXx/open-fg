#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# ---------------------------------------------------------------------------
# Load local Android env config if present
# ---------------------------------------------------------------------------
ENV_FILE="${ROOT_DIR}/.env.android.local"
if [[ -f "${ENV_FILE}" ]]; then
  # shellcheck disable=SC1090
  source "${ENV_FILE}"
fi

ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-}"
ANDROID_API_LEVEL="${ANDROID_API_LEVEL:-21}"

if [[ -z "${ANDROID_NDK_ROOT}" ]]; then
  # Try common default locations
  for candidate in \
    "${HOME}/Library/Android/sdk/ndk/27.3.13750724" \
    "/opt/homebrew/share/android-commandlinetools/ndk/22.1.7171670" \
    "${ANDROID_HOME:-}/ndk-bundle"; do
    if [[ -d "${candidate}/toolchains/llvm" ]]; then
      ANDROID_NDK_ROOT="${candidate}"
      break
    fi
  done
fi

if [[ -z "${ANDROID_NDK_ROOT}" ]]; then
  echo "error: ANDROID_NDK_ROOT not set and no NDK found in default locations." >&2
  echo "       Copy .env.android.local.example to .env.android.local and set ANDROID_NDK_ROOT." >&2
  exit 1
fi

# Detect host platform (darwin or linux)
HOST_OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "${HOST_OS}" in
  darwin) HOST_TAG="darwin-x86_64" ;;
  linux)  HOST_TAG="linux-x86_64"  ;;
  *)
    echo "error: unsupported host OS: ${HOST_OS}" >&2
    exit 1
    ;;
esac

NDK_BIN="${ANDROID_NDK_ROOT}/toolchains/llvm/prebuilt/${HOST_TAG}/bin"
CLANG="${NDK_BIN}/aarch64-linux-android${ANDROID_API_LEVEL}-clang"
AR="${NDK_BIN}/llvm-ar"

if [[ ! -x "${CLANG}" ]]; then
  echo "error: clang not found at ${CLANG}" >&2
  echo "       Check ANDROID_NDK_ROOT and ANDROID_API_LEVEL in .env.android.local." >&2
  exit 1
fi

echo "==> Android Bionic arm64 build"
echo "    NDK:    ${ANDROID_NDK_ROOT}"
echo "    clang:  ${CLANG}"
echo "    API:    ${ANDROID_API_LEVEL}"
echo ""

OUT_DIR="${ROOT_DIR}/build/android-bionic/vk-layer-rust/out"
mkdir -p "${OUT_DIR}"

# Ensure the Rust target is available
rustup target add aarch64-linux-android 2>/dev/null || true

# Prefer the rustup-managed cargo/rustc to avoid conflicts with system Rust installs
# (e.g. Homebrew Rust on macOS that shadows rustup's toolchain)
if command -v rustup &>/dev/null; then
  CARGO="$(rustup which cargo 2>/dev/null)"
  RUSTC="$(rustup which rustc 2>/dev/null)"
else
  CARGO="cargo"
  RUSTC="rustc"
fi

# Build — use env vars for the machine-specific linker/ar so config.toml stays portable
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${CLANG}" \
CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="${AR}" \
RUSTC="${RUSTC}" \
  "${CARGO}" build --release --target aarch64-linux-android

SO="${ROOT_DIR}/target/aarch64-linux-android/release/libVkLayer_OMFG_rust.so"
MANIFEST="${ROOT_DIR}/manifest/VkLayer_OMFG_rust.json"

cp "${SO}"       "${OUT_DIR}/libVkLayer_OMFG_rust.so"
cp "${MANIFEST}" "${OUT_DIR}/VkLayer_OMFG_rust.json"

echo ""
echo "==> Output:"
ls -lah "${OUT_DIR}"
echo ""
echo "==> ELF check:"
file "${OUT_DIR}/libVkLayer_OMFG_rust.so"

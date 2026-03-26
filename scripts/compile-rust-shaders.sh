#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE_TAG="ppfg-linux-amd64-builder:latest"
SHADER_DIR="${ROOT_DIR}/implementation/vk-layer-rust/shaders"

compile_with_local_tool() {
  glslangValidator -V "${SHADER_DIR}/blend.vert" -o "${SHADER_DIR}/blend.vert.spv"
  glslangValidator -V "${SHADER_DIR}/blend.frag" -o "${SHADER_DIR}/blend.frag.spv"
}

compile_with_docker() {
  docker build \
    --platform linux/amd64 \
    -t "${IMAGE_TAG}" \
    -f "${ROOT_DIR}/docker/linux-amd64-builder.Dockerfile" \
    "${ROOT_DIR}"

  docker run --rm \
    --platform linux/amd64 \
    -v "${ROOT_DIR}:/workspace" \
    -w /workspace \
    "${IMAGE_TAG}" \
    bash -lc '
      set -euo pipefail
      glslangValidator -V implementation/vk-layer-rust/shaders/blend.vert -o implementation/vk-layer-rust/shaders/blend.vert.spv
      glslangValidator -V implementation/vk-layer-rust/shaders/blend.frag -o implementation/vk-layer-rust/shaders/blend.frag.spv
    '
}

if command -v glslangValidator >/dev/null 2>&1; then
  compile_with_local_tool
else
  compile_with_docker
fi

echo "Compiled Rust layer shaders under ${SHADER_DIR}"

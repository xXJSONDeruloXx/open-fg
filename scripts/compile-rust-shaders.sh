#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE_TAG="omfg-linux-amd64-builder:latest"
SHADER_DIR="${ROOT_DIR}/shaders"

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
      glslangValidator -V shaders/blend.vert -o shaders/blend.vert.spv
      glslangValidator -V shaders/blend.frag -o shaders/blend.frag.spv
    '
}

if command -v glslangValidator >/dev/null 2>&1; then
  compile_with_local_tool
else
  compile_with_docker
fi

echo "Compiled Rust layer shaders under ${SHADER_DIR}"

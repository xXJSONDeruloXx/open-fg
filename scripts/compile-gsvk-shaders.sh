#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SHADER_DIR="${ROOT_DIR}/shaders/gamescopevk"

echo "Compiling GameScopeVK compute shaders..."

for f in "${SHADER_DIR}"/shader_*.comp; do
  base="${f%.comp}"
  glslangValidator -V --target-env vulkan1.1 "$f" -o "${base}.spv"
done

echo "Compiled GameScopeVK compute shaders under ${SHADER_DIR}"

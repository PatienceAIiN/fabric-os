#!/usr/bin/env bash
# Source to enable real local inference (GPU if the Vulkan build exists).
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -x "$HERE/build/llama.cpp/build-vk/bin/llama-cli" ]; then
  export AIOS_LLAMA_CLI="$HERE/build/llama.cpp/build-vk/bin/llama-cli"
  export AIOS_LLAMA_NGL=99   # offload all layers to the GPU (Vulkan)
  echo "local AI enabled: GPU (Vulkan) inference"
else
  export AIOS_LLAMA_CLI="$HERE/build/llama.cpp/build/bin/llama-cli"
  export AIOS_LLAMA_NGL=0
  echo "local AI enabled: CPU inference"
fi
export AIOS_LLAMA_MODEL="$HERE/models/weights/qwen2.5-0.5b-instruct-q4_k_m.gguf"

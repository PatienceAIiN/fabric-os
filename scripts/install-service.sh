#!/usr/bin/env bash
# install-service.sh — install aiosd as a user service (System-Wide AI = ON).
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
"$HERE/tools/rg" --profile build -- cargo build --release -p aiosd >/dev/null 2>&1
install -Dm755 "$HERE/target/release/aiosd" "$HOME/.local/bin/aiosd"
install -Dm644 "$HERE/systemd/user/aiosd.service" "$HOME/.config/systemd/user/aiosd.service"
# wire real local model if built
if [ -x "$HERE/build/llama.cpp/build/bin/llama-cli" ]; then
  install -Dm755 "$HERE/build/llama.cpp/build/bin/llama-cli" "$HOME/.local/share/ai-native-os/llama-cli"
  mkdir -p "$HOME/.local/share/ai-native-os/models"
  cp -n "$HERE"/models/weights/*.gguf "$HOME/.local/share/ai-native-os/models/" 2>/dev/null || true
fi
systemctl --user daemon-reload
echo "installed. Turn System-Wide AI ON:  systemctl --user enable --now aiosd"

# Testing Fabric OS

## 1. Fabric OS desktop on your machine (fastest)

```
cd ~/ai-native-os
source scripts/ai-env.sh        # enable local AI (GPU if available)
./scripts/fabric-os.sh          # opens the desktop (fullscreen kiosk)
./scripts/fabric-os.sh app      # or a normal window
```

Splash → login (**admin / fabric**) → desktop. Dock apps:
- **AI Assistant** — ask anything (local inference); "Organize Downloads →
  intent" drafts an intent for the Governor.
- **Agents / Intents** — full create / list / revoke / authorize / cancel.
- **Governor** — request a capability; out-of-intent requests are BLOCKED.
- **Activity** — the hash-chained provenance of every decision.
- **Monitor** — live CPU / RAM / GPU. **Settings** — appearance / AI / privacy.

## 2. Boot the whole OS in a VM (desktop served by the OS)

```
./scripts/launch-os.sh
```

A QEMU window shows the OS console; the OS boots, starts its desktop server,
and your browser opens the Fabric OS desktop at `http://localhost:8788`
(served by the booted OS). Login: admin / fabric.

## 3. Automated verification

```
./scripts/build-all.sh          # host check, 96 tests, 2 QEMU boots, Wayland, sandbox
./scripts/boot-disk.sh          # boots the OS; asserts in-guest security demos
```


## Apps (every action is a real system operation)

- **Files** — real filesystem (workspace ~/FabricOS): create/rename/trash/
  restore/copy/move/properties/search; open text in Editor, images in viewer.
- **Terminal** — real `sh` command execution with cwd tracking.
- **Text Editor** — real open/save to disk.
- **Processes** — real `/proc` list, filter, kill (with confirmation).
- **System Monitor** — real CPU/RAM/swap/load/GPU with live sparklines.
- **System Info** — real uname/os-release/cpu/disk.
- **Settings** — Appearance (theme/accent/wallpaper/font/reduced-motion, live +
  persisted), Account (password/username change, hashed), AI, and real read-only
  Network / Storage / Users / Display / Date&Time / Power / About.
- **Trash / Clipboard / Notifications** — real CRUD.
- **AI Assistant** — natural language → real OS actions (open folder, create
  folder, search, top-RAM, disk usage, open terminal); dangerous ops confirm.
- **Governor / Agents / Intents** — the deterministic security core.

Window manager: multi-window, drag, resize, min/max/snap, taskbar, workspaces,
launcher (Activities / Super), context menus, keyboard shortcuts.

## Notes
- All heavy builds run under `tools/rg` so the host stays responsive.
- Local AI: llama.cpp + Qwen2.5-0.5B, CPU or GPU (Vulkan). Without it, the
  assistant falls back to a deterministic stub.
- Screenshots: `docs/design/screenshots/fabric-*.png`, `fabric-os-booted.png`.

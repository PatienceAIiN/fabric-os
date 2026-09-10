# System-wide AI service (M10)

`aiosd.service` is a systemd **user** unit (no root). Enabling it is exactly the
"System-Wide AI = ON" action:

```
# install binary + unit
install -Dm755 target/release/aiosd ~/.local/bin/aiosd
install -Dm644 systemd/user/aiosd.service ~/.config/systemd/user/aiosd.service
systemctl --user daemon-reload

# turn System-Wide AI ON / OFF
systemctl --user enable --now aiosd      # ON
systemctl --user disable --now aiosd     # OFF
systemctl --user status aiosd
```

When OFF, the socket is gone and AI integrations are disabled; normal Linux
operation is unaffected (spec §63). The unit is sandboxed (NoNewPrivileges,
ProtectSystem=strict, ProtectHome=read-only), so a compromised service cannot
roam the filesystem. The Claude key is resolved only inside the service via
libcredentials; it is never in the unit file or environment.

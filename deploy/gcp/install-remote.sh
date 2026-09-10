#!/usr/bin/env bash
set -Eeuo pipefail

APP_ROOT=/opt/fabric-os
sudo install -d -m 0755 "$APP_ROOT" /opt/fabric-os/build
sudo groupadd --system fabricos 2>/dev/null || true
sudo useradd --system --gid fabricos --home-dir "$APP_ROOT" --shell /usr/sbin/nologin fabricos 2>/dev/null || true

sudo rm -rf "$APP_ROOT/website"
sudo install -d -o fabricos -g fabricos "$APP_ROOT/website"
sudo install -d -o fabricos -g fabricos "$APP_ROOT/.npm"
sudo tar -xzf /tmp/fabric-os-website.tgz -C "$APP_ROOT/website" --strip-components=1
sudo chown -R fabricos:fabricos "$APP_ROOT/website"

cd "$APP_ROOT/website"
sudo -u fabricos env NPM_CONFIG_CACHE="$APP_ROOT/.npm" /usr/bin/npm ci --omit=dev
sudo -u fabricos env NPM_CONFIG_CACHE="$APP_ROOT/.npm" /usr/bin/npm run build

sudo install -m 0644 /tmp/fabric-os-web.service /etc/systemd/system/fabric-os-web.service
sudo install -m 0644 /tmp/fabricos.patienceai.in.nginx /etc/nginx/sites-available/fabricos.patienceai.in
sudo ln -sfn /etc/nginx/sites-available/fabricos.patienceai.in /etc/nginx/sites-enabled/fabricos.patienceai.in

if [[ -f /tmp/rootfs.ext4 ]]; then
    echo "OS image upload detected; refusing to install an under-development image."
    exit 1
fi

sudo bash /tmp/bootstrap-db.sh
sudo systemctl daemon-reload
sudo systemctl enable --now fabric-os-web.service
sudo nginx -t
sudo systemctl reload nginx

curl --fail --silent --show-error http://127.0.0.1:4173/healthz
echo
curl --fail --silent --show-error -I http://127.0.0.1:4173/ | head -20

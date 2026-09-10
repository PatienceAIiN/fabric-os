#!/usr/bin/env bash
set -Eeuo pipefail

DB_PASSWORD="$(openssl rand -hex 32)"
DB_NAME="fabricos"
DB_USER="fabricos_app"

if sudo -u postgres psql -Atqc "SELECT 1 FROM pg_roles WHERE rolname='${DB_USER}'" | grep -q 1; then
  sudo -u postgres psql -v ON_ERROR_STOP=1 -c "ALTER ROLE ${DB_USER} PASSWORD '${DB_PASSWORD}'"
else
  sudo -u postgres psql -v ON_ERROR_STOP=1 -c "CREATE ROLE ${DB_USER} LOGIN PASSWORD '${DB_PASSWORD}'"
fi

if ! sudo -u postgres psql -Atqc "SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'" | grep -q 1; then
  sudo -u postgres createdb -O "${DB_USER}" "${DB_NAME}"
fi

sudo -u postgres psql -v ON_ERROR_STOP=1 -d "${DB_NAME}" -f /opt/fabric-os/website/db/schema.sql

sudo install -d -m 0750 -o root -g fabricos /etc/fabric-os
tmp_env="$(mktemp)"
trap 'rm -f "$tmp_env"' EXIT
printf 'NODE_ENV=production\nPORT=4173\nDATABASE_SSL=false\nDATABASE_URL=postgresql://%s:%s@127.0.0.1:5432/%s\n' "${DB_USER}" "${DB_PASSWORD}" "${DB_NAME}" > "$tmp_env"
sudo install -m 0640 -o root -g fabricos "$tmp_env" /etc/fabric-os/fabric-os.env

#!/usr/bin/env bash
# Runs on the VPS. The archive contains only public build artifacts and deploy files.
set -euo pipefail
stage=${1:?staging directory required}
[[ "$stage" == /tmp/spider-vk-release.* ]] || exit 2
cd "$stage"
revision=$(sha256sum web/dist/*_bg.wasm | cut -c1-12)
image="spider-vk-web:local-$revision"
api_revision=$(cat Cargo.lock server/src/*.rs engine/src/*.rs | sha256sum | cut -c1-12)
api_image="spider-vk-api:local-$api_revision"
app=/opt/bots/bots/spider-vk
backup="/opt/bots/backups/spider-vk-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$app" "$backup"
cp -p /etc/caddy/Caddyfile "$backup/Caddyfile.before"
if [ -f "$app/.api-image" ]; then cp "$app/.api-image" "$backup/api-image.before"; fi
if [ -f "$app/.image" ]; then cp "$app/.image" "$backup/image.before"; fi
if [ -f "$app/compose.yaml" ]; then cp "$app/compose.yaml" "$backup/compose.before.yaml"; fi
if ss -ltn | grep -q '127.0.0.1:8094 ' && ! docker ps --format '{{.Names}}' | grep -qx spider-vk-web; then
  echo 'Port 8094 belongs to another service' >&2; exit 1
fi
docker build -f deploy/Dockerfile.api -t "$api_image" .
docker build -f deploy/Dockerfile.prebuilt -t "$image" .
install -m 644 deploy/vk-bots-server/bots/spider-vk/compose.yaml "$app/compose.yaml"
install -m 644 deploy/vk-bots-server/bots/spider-vk/bot.conf "$app/bot.conf"
install -m 644 deploy/vk-bots-server/bots/spider-vk/.env.example "$app/.env.example"
if [ ! -f "$app/.env" ]; then install -m 600 "$app/.env.example" "$app/.env"; fi
rollback() {
  if [ -f "$backup/image.before" ]; then
    cp "$backup/image.before" "$app/.image"
    if [ -f "$backup/compose.before.yaml" ]; then cp "$backup/compose.before.yaml" "$app/compose.yaml"; fi
    previous_api=spider-vk-api:local
    if [ -f "$backup/api-image.before" ]; then previous_api=$(cat "$backup/api-image.before");cp "$backup/api-image.before" "$app/.api-image";docker tag "$previous_api" spider-vk-api:local; fi
    API_IMAGE="$previous_api" IMAGE=$(cat "$app/.image") docker compose -f "$app/compose.yaml" up -d --remove-orphans
  else
    IMAGE="$image" docker compose -f "$app/compose.yaml" down
  fi
  cp -p "$backup/Caddyfile.before" /etc/caddy/Caddyfile
  systemctl reload caddy
}
trap 'rollback' ERR
if docker ps --format '{{.Names}}' | grep -qx spider-vk-api; then
  docker exec spider-vk-api /usr/local/bin/spider-server --backup "/data/backups/pre-deploy-$(date -u +%Y%m%dT%H%M%SZ).sqlite3"
fi
API_IMAGE="$api_image" IMAGE="$image" docker compose -f "$app/compose.yaml" up -d --wait --wait-timeout 60
curl -fsS --max-time 10 http://127.0.0.1:8094/healthz
# Patch the LIVE config: never apply pending blocks belonging to other games.
python3 - "$stage/deploy/vk-bots-server/Caddyfile.fragment" <<'PY'
from pathlib import Path
import sys
p=Path('/etc/caddy/Caddyfile')
s=p.read_text(); block=Path(sys.argv[1]).read_text().strip()
begin='# BEGIN spider-vk (managed from spider-game/deploy/vk-bots-server)'; end='# END spider-vk'
if begin in s:
    a=s.index(begin);b=s.index(end,a)+len(end);s=s[:a]+block+s[b:]
else:
    s=s.rstrip()+'\n\n'+block+'\n'
Path('/etc/caddy/Caddyfile.spider-new').write_text(s)
PY
caddy validate --config /etc/caddy/Caddyfile.spider-new --adapter caddyfile
mv /etc/caddy/Caddyfile.spider-new /etc/caddy/Caddyfile
systemctl reload caddy
systemctl is-active --quiet caddy
if [ -f "$backup/image.before" ] && [ "$(cat "$backup/image.before")" != "$(docker image inspect "$image" --format '{{.Id}}')" ]; then
  cp "$backup/image.before" "$app/.image.prev"
fi
docker image inspect "$image" --format '{{.Id}}' > "$app/.image"
docker tag "$api_image" spider-vk-api:local
docker image inspect "$api_image" --format '{{.Id}}' > "$app/.api-image"
trap - ERR
echo "DEPLOYED https://spider.62-109-4-100.sslip.io image=$image backup=$backup"

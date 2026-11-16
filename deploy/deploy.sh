#!/usr/bin/env bash
# Ship a verified release as an isolated nginx Docker image. Never transfers .env.
set -euo pipefail
cd "$(dirname "$0")/.."
test -f web/dist/index.html || { echo 'Build first: cd web && trunk build --release --locked' >&2; exit 1; }
server=${SPIDER_SSH_HOST:-root@85.137.95.37}
ssh_options=(-o BatchMode=yes -o ConnectTimeout=20 -o ServerAliveInterval=15)
/usr/bin/python3 deploy/configure-api.py
stage=$(ssh "${ssh_options[@]}" "$server" 'mktemp -d /tmp/spider-release.XXXXXXXX')
[[ "$stage" == /tmp/spider-release.* ]] || exit 2
COPYFILE_DISABLE=1 tar --no-xattrs -czf - web/dist web/Cargo.toml Cargo.toml Cargo.lock engine server deploy/nginx.conf deploy/Dockerfile.prebuilt deploy/Dockerfile.api deploy/bots-server deploy/remote-deploy.sh | ssh "${ssh_options[@]}" "$server" "tar -xzf - -C '$stage'"
ssh "${ssh_options[@]}" "$server" "bash '$stage/deploy/remote-deploy.sh' '$stage'"
echo 'Verify external HTTPS: curl -fsS https://spider.85-137-95-37.sslip.io/healthz'

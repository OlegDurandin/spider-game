#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
test -f web/dist/index.html
server=${SPIDER_VK_SSH_HOST:-root@62.109.4.100}
ssh_options=(-o BatchMode=yes -o ConnectTimeout=20 -o ServerAliveInterval=15)
/usr/bin/python3 deploy/vk/configure.py
stage=$(ssh "${ssh_options[@]}" "$server" 'mktemp -d /tmp/spider-vk-release.XXXXXXXX')
[[ "$stage" == /tmp/spider-vk-release.* ]]
COPYFILE_DISABLE=1 tar --no-xattrs -czf - web/dist web/Cargo.toml Cargo.toml Cargo.lock engine server deploy/nginx.conf deploy/Dockerfile.prebuilt deploy/Dockerfile.api deploy/vk-bots-server deploy/vk/remote.sh | ssh "${ssh_options[@]}" "$server" "tar -xzf - -C '$stage'"
ssh "${ssh_options[@]}" "$server" "bash '$stage/deploy/vk/remote.sh' '$stage'"

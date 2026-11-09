#!/usr/bin/env bash
# Resolve this script's real directory so .env and the ../.. compose build
# context are always consistent, even under "sudo bash /path/start.sh" where
# $0/$PWD are not reliable.
set -euo pipefail
cd "$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-$0}")" && pwd -P)"
if [[ ! -f .env ]]; then
  relay_domain="${1:?Usage: bash deploy/network-relay/start.sh relay.your-domain.tld}"
  if [[ "$relay_domain" != *.* || "$relay_domain" == *[^a-zA-Z0-9.-]* || "$relay_domain" == *example.* ]]; then
    echo 'Supply your real public DNS hostname, without https:// or a path.' >&2
    exit 1
  fi
  umask 077
  relay_proxy_token="$(openssl rand -hex 32)"
  cat > .env <<EOF
RELAY_DOMAIN=$relay_domain
RELAY_ORIGINS=https://zhuzhe1983.github.io
RELAY_PROXY_TOKEN=$relay_proxy_token
EOF
fi
docker compose --env-file .env up --build -d
docker compose --env-file .env ps

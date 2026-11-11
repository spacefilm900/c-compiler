#!/bin/sh
# Install CI packages. GitHub's ubuntu-24.04 runners list
# azure.archive.ubuntu.com first in /etc/apt/apt-mirrors.txt; when that
# name does not resolve, apt-get update and install fail with exit 100.
# Prefer the public Ubuntu archive, keep the Azure mirror as a fallback,
# and retry a few times.
set -eu

prefer_public_mirror() {
  if [ -f /etc/apt/apt-mirrors.txt ]; then
    sudo tee /etc/apt/apt-mirrors.txt >/dev/null <<'MIRRORS'
http://archive.ubuntu.com/ubuntu/	priority:1
http://security.ubuntu.com/ubuntu/	priority:2
http://azure.archive.ubuntu.com/ubuntu/	priority:3
MIRRORS
  fi
}

apt_retry() {
  i=1
  while [ "$i" -le 5 ]; do
    if sudo apt-get "$@"; then
      return 0
    fi
    prefer_public_mirror
    sleep $((i * 5))
    i=$((i + 1))
  done
  return 1
}

prefer_public_mirror
# --error-on=any makes a partial index download a hard failure, so the
# retry loop can try again instead of treating a warning as success.
apt_retry update --error-on=any
apt_retry install -y --no-install-recommends "$@"

#!/bin/sh
# Install CI packages. GitHub's ubuntu-24.04 runners often list
# azure.archive.ubuntu.com in /etc/apt/apt-mirrors.txt; when that name
# does not resolve, apt-get update and install fail with exit 100.
# Point apt at the public Ubuntu archive first, then retry a few times.
set -eu

prefer_public_mirror() {
  if [ -f /etc/apt/apt-mirrors.txt ]; then
    sudo sed -i 's/azure.archive.ubuntu.com/archive.ubuntu.com/g' /etc/apt/apt-mirrors.txt
  fi
  if [ -f /etc/apt/sources.list ]; then
    sudo sed -i 's/azure.archive.ubuntu.com/archive.ubuntu.com/g' /etc/apt/sources.list
  fi
  if [ -d /etc/apt/sources.list.d ]; then
    sudo find /etc/apt/sources.list.d -type f \
      -exec sed -i 's/azure.archive.ubuntu.com/archive.ubuntu.com/g' {} +
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
apt_retry update
apt_retry install -y --no-install-recommends "$@"

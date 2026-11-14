#!/bin/sh
# Prefer tools already on the GitHub-hosted ubuntu-24.04 image
# (Clang 16–18, make, python3, bison, g++, unzip). Only call apt-get for
# something that is actually missing, so a broken apt mirror does
# not fail the job. If apt is needed, prefer archive.ubuntu.com and
# keep the Azure mirror as a fallback, and retry a few times.
set -eu

# Link a versioned tool (clang-18, llvm-as-18, ...) to the unversioned name.
# Only LLVM 18 or newer: older x86-64 backends split an i128 argument
# between the last GP register and the stack (see README).
find_tool() {
  cmd=$1
  if command -v "$cmd" >/dev/null 2>&1; then
    major=$("$cmd" --version 2>/dev/null |
      sed -n 's/.*version \([0-9][0-9]*\)\..*/\1/p' | head -n 1) || major=
    if [ -n "$major" ] && [ "$major" -ge 18 ]; then
      return 0
    fi
  fi
  ver=
  for ver in 18 19 20 21; do
    if command -v "$cmd-$ver" >/dev/null 2>&1; then
      sudo ln -sf "$(command -v "$cmd-$ver")" /usr/local/bin/"$cmd"
      return 0
    fi
  done
  return 1
}

need=
for pkg in "$@"; do
  case "$pkg" in
    clang)
      find_tool clang || need="$need clang"
      ;;
    llvm)
      find_tool llvm-as || need="$need llvm"
      # harness/run_arm64_ir.py also finds a versioned llc-18..21 itself.
      find_tool llc || :
      ;;
    make)
      command -v make >/dev/null 2>&1 || need="$need make"
      ;;
    python3)
      command -v python3 >/dev/null 2>&1 || need="$need python3"
      ;;
    bison)
      command -v bison >/dev/null 2>&1 || need="$need bison"
      ;;
    g++)
      command -v g++ >/dev/null 2>&1 || need="$need g++"
      ;;
    unzip)
      command -v unzip >/dev/null 2>&1 || need="$need unzip"
      ;;
    *)
      need="$need $pkg"
      ;;
  esac
done
need=${need# }

if [ -z "$need" ]; then
  echo "All requested tools are already on the runner; skipping apt-get."
  exit 0
fi

echo "Installing missing packages: $need"

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
# Restrict that strict update to Ubuntu archives. A failed third-party
# repository on the runner must not block installing these packages.
ubuntu_lists=$(mktemp -d)
mkdir -p "$ubuntu_lists/sources.list.d"
if [ -f /etc/apt/sources.list ]; then
  grep -E '^[[:space:]]*(#|$)|ubuntu\.com' /etc/apt/sources.list >"$ubuntu_lists/sources.list" || :
fi
if [ ! -s "$ubuntu_lists/sources.list" ]; then
  : >"$ubuntu_lists/sources.list"
fi
if [ -d /etc/apt/sources.list.d ]; then
  for src in /etc/apt/sources.list.d/*; do
    [ -f "$src" ] || continue
    case "$(basename "$src")" in
      ubuntu.sources|ubuntu.sources.*|ubuntu-*.sources|ubuntu.list|ubuntu-*.list)
        cp "$src" "$ubuntu_lists/sources.list.d/"
        ;;
    esac
  done
fi
# shellcheck disable=SC2064
trap 'rm -rf "$ubuntu_lists"' EXIT
apt_retry update --error-on=any \
  -o Dir::Etc::sourcelist="$ubuntu_lists/sources.list" \
  -o Dir::Etc::sourceparts="$ubuntu_lists/sources.list.d"
# shellcheck disable=SC2086
apt_retry install -y --no-install-recommends $need

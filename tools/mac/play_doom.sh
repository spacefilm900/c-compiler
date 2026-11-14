#!/bin/bash
# Build DOOM with this repository's C compiler and play it in a window.
#
#   ./tools/mac/play_doom.sh                    play Freedoom (free game data)
#   ./tools/mac/play_doom.sh --wad ~/DOOM.WAD   play your own DOOM WAD
#   ./tools/mac/play_doom.sh --build-only       build, but do not start
#
# Anything else on the command line goes to the game (for example
# -warp 1 1 or -skill 2). docs/DOOM_ON_MAC.md explains every step.
#
# The script checks what it needs first (Xcode Command Line Tools,
# Homebrew, Rust, SDL2) and says how to install whatever is missing. Then
# it builds the compiler, fetches the DOOM source (doomgeneric, at a pinned
# commit) and Freedoom (checked against its SHA-256), compiles every DOOM
# C file and the window code with this compiler, links them with SDL2, and
# starts the game.
#
# Every C file is compiled by build/compiler into LLVM IR text; Apple's
# clang then turns that IR into machine code and links the program, the
# way the rest of this project uses it (clang never sees the C source).
#
# Settings for other systems and for CI: CLANG (the IR assembler and
# linker), SDL_CFLAGS and SDL_LIBS (instead of sdl2-config), DOOM_OPT
# (optimisation level for the IR, default O2), DOOM_BUILD (output folder).
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$ROOT"

say() { printf '%s\n' "$*"; }
step() { printf '\n==> %s\n' "$*"; }

BUILD_ONLY=0
WAD=""
GAME_ARGS=()
while [ $# -gt 0 ]; do
    case "$1" in
        --build-only) BUILD_ONLY=1 ;;
        --wad)
            [ $# -ge 2 ] || { say "--wad needs a file name, for example --wad ~/Documents/DOOM.WAD"; exit 2; }
            WAD=$2
            shift
            ;;
        --wad=*) WAD=${1#--wad=} ;;
        -h | --help)
            sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) GAME_ARGS+=("$1") ;;
    esac
    shift
done

# --- 1. Check what is needed ------------------------------------------------

step "Checking what this Mac needs"
MISSING=0
missing() {
    MISSING=1
    printf '\n  MISSING: %s\n' "$1"
    shift
    for line in "$@"; do
        printf '    %s\n' "$line"
    done
}

# Homebrew and Rust install into folders a new Terminal window may not
# search yet; look there too.
for dir in /opt/homebrew/bin /usr/local/bin "$HOME/.cargo/bin"; do
    case ":$PATH:" in
        *":$dir:"*) ;;
        *) [ -d "$dir" ] && PATH="$PATH:$dir" ;;
    esac
done
export PATH

if [ "$(uname -s)" = Darwin ]; then
    if [ "$(uname -m)" != arm64 ]; then
        say "  Note: this Mac is not Apple Silicon ($(uname -m)). The guide is"
        say "  written for Apple Silicon; the build may still work."
    fi
    if ! xcode-select -p >/dev/null 2>&1 || ! /usr/bin/xcrun --find clang >/dev/null 2>&1; then
        missing "Apple's Xcode Command Line Tools (the C toolchain and git)" \
            "Type this, press Return, and click Install in the window that opens:" \
            "    xcode-select --install" \
            "When it says it is done, run this script again."
    fi
    if ! command -v brew >/dev/null 2>&1; then
        missing "Homebrew (installs SDL2, the window and keyboard library)" \
            "Copy this line into Terminal and press Return (it asks for your Mac password):" \
            '    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"' \
            "Then run the two commands it prints under \"Next steps\", and run this script again."
    fi
fi
if ! command -v cargo >/dev/null 2>&1; then
    missing "Rust (builds the C compiler, which is written in Rust)" \
        "Copy this line into Terminal, press Return, then press Return again to accept the defaults:" \
        "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" \
        "Then close Terminal, open it again, and run this script again."
fi

if [ -z "${SDL_CFLAGS:-}" ] || [ -z "${SDL_LIBS:-}" ]; then
    SDL_CONFIG=""
    if command -v sdl2-config >/dev/null 2>&1; then
        SDL_CONFIG=$(command -v sdl2-config)
    elif command -v brew >/dev/null 2>&1; then
        prefix=$(brew --prefix 2>/dev/null || true)
        [ -n "$prefix" ] && [ -x "$prefix/bin/sdl2-config" ] && SDL_CONFIG="$prefix/bin/sdl2-config"
    fi
    if [ -n "$SDL_CONFIG" ]; then
        SDL_CFLAGS=$("$SDL_CONFIG" --cflags)
        SDL_LIBS=$("$SDL_CONFIG" --libs)
    else
        missing "SDL2 (opens the game window and reads the keyboard)" \
            "Type this and press Return:" \
            "    brew install sdl2" \
            "Then run this script again."
    fi
fi

if [ "$MISSING" = 1 ]; then
    say ""
    say "Install what is listed above, then run ./tools/mac/play_doom.sh again."
    exit 1
fi

if [ -z "${CLANG:-}" ]; then
    if [ "$(uname -s)" = Darwin ]; then
        CLANG=/usr/bin/clang
        # Apple's clang finds the macOS system files through SDKROOT.
        if [ -z "${SDKROOT:-}" ]; then
            SDKROOT=$(/usr/bin/xcrun --show-sdk-path)
            export SDKROOT
        fi
    else
        CLANG=clang
    fi
fi
OPT=${DOOM_OPT:-O2}
say "  Everything needed is installed."

# --- 2. Build the compiler and fetch the game ------------------------------

step "Building the C compiler (the first time takes a minute or two)"
make -s build/compiler
COMPILER="$ROOT/build/compiler"

step "Fetching the DOOM source code (doomgeneric) and the Freedoom game data"
sh scripts/fetch_doomgeneric.sh
sh scripts/fetch_freedoom.sh

SRC="$ROOT/tests/external/doomgeneric/doomgeneric"
OUT=${DOOM_BUILD:-"$ROOT/build/doom-sdl"}
mkdir -p "$OUT"
# Absolute, because the game is started from its saves folder.
OUT=$(CDPATH= cd -- "$OUT" && pwd)

# The upstream Makefile's list of DOOM files, minus its X11 window code;
# harness/doom/dg_sdl.c (this repository) is the window code instead.
NAMES=$(sed -n 's/^SRC_DOOM[[:space:]]*=[[:space:]]*//p' "$SRC/Makefile")
if [ -z "$NAMES" ]; then
    say "Cannot read the list of DOOM files from $SRC/Makefile"
    exit 1
fi

# --- 3. Compile DOOM with this compiler -------------------------------------

step "Compiling DOOM with this project's compiler"
DEFINES="-DNORMALUNIX -D_DEFAULT_SOURCE"
# Objects are reused only if they were built the same way: a different
# optimisation level, SDL, or Clang rebuilds everything.
CONFIG="opt=$OPT clang=$CLANG defines=$DEFINES sdl=$SDL_CFLAGS"
if [ "$(cat "$OUT/config" 2>/dev/null || true)" != "$CONFIG" ]; then
    rm -f "$OUT"/*.o "$OUT"/*.ll
    printf '%s\n' "$CONFIG" > "$OUT/config"
fi
compiled=0
objects=()
compile() {
    src=$1
    shift
    base=$(basename "$src" .c)
    ll="$OUT/$base.ll"
    obj="$OUT/$base.o"
    objects+=("$obj")
    if [ -f "$obj" ] && [ "$obj" -nt "$src" ] && [ "$obj" -nt "$COMPILER" ]; then
        return 0
    fi
    # shellcheck disable=SC2086 # DEFINES and the SDL flags are word lists.
    if ! "$COMPILER" -std=gnu11 $DEFINES "$@" -I "$SRC" "$src" -o "$ll"; then
        say "This compiler could not compile $src (see the message above)."
        say "Please report it, with the message, on the project's GitHub page."
        exit 1
    fi
    "$CLANG" "-$OPT" -w -c "$ll" -o "$obj"
    compiled=$((compiled + 1))
}
for name in $NAMES; do
    [ "$name" = doomgeneric_xlib.o ] && continue
    compile "$SRC/${name%.o}.c"
done
# shellcheck disable=SC2086
compile "$ROOT/harness/doom/dg_sdl.c" $SDL_CFLAGS
say "  ${#objects[@]} C files, $compiled compiled now, the rest unchanged since the last build."

step "Linking"
# shellcheck disable=SC2086
"$CLANG" -o "$OUT/doom" "${objects[@]}" $SDL_LIBS -lm
say "  Built $OUT/doom"

if [ "$BUILD_ONLY" = 1 ]; then
    exit 0
fi

# --- 4. Play -----------------------------------------------------------------

if [ -z "$WAD" ]; then
    WAD="$ROOT/tests/external/freedoom/freedoom1.wad"
else
    case "$WAD" in
        "~/"*) WAD="$HOME/${WAD#\~/}" ;;
    esac
    if [ ! -f "$WAD" ]; then
        say "Cannot find the WAD file $WAD"
        exit 1
    fi
    WAD=$(CDPATH= cd -- "$(dirname -- "$WAD")" && pwd)/$(basename -- "$WAD")
fi

# DOOM keeps its settings (default.cfg) and saved games in the folder it
# starts in.
SAVES="$OUT/saves"
mkdir -p "$SAVES"
cd "$SAVES"

step "Starting DOOM with $(basename "$WAD")"
say "  Click the DOOM window if it is behind Terminal. Esc opens the menu."
say "  To quit: Esc, choose Quit Game, press Y. (Or press Cmd+Q, or close the window.)"
say "  Settings and saved games are kept in $SAVES"
exec "$OUT/doom" -iwad "$WAD" ${GAME_ARGS[@]+"${GAME_ARGS[@]}"}

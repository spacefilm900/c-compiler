# Play DOOM on your Mac, built with this compiler

This guide is for an Apple Silicon Mac (a MacBook Air or any Mac with an
M1, M2, M3, M4 or later chip). You do not need to know how to program. You
will copy a few lines into the Terminal app and press Return.

One command does the work: it checks your Mac has what it needs, builds
this project's C compiler, uses it to compile DOOM, and opens the game in a
window.

> **Status.** The build and the game have run on GitHub's Apple Silicon
> Mac (macOS 26): `./tools/mac/play_doom.sh` built DOOM, opened its
> window, and scripted key presses started the first level. Nobody has
> played it by hand on their own Mac yet. If a step goes wrong, see
> [If something goes wrong](#if-something-goes-wrong).

**How long:** about 15 to 30 minutes the first time, mostly downloads.
After that, starting the game takes a few seconds.

**What you will need:** an internet connection, your Mac's login password
(to install the tools), and about 5 GB of free disk space.

## About the game data

DOOM is two parts: the program (the "engine", whose source code id
Software released for free) and the game data (the levels, pictures and
sounds), which is a file ending in `.wad`.

The script uses **Freedoom**, a free replacement for DOOM's game data made
by volunteers. It looks and plays like DOOM with different art and levels.
The script downloads it and checks the file is exactly the expected one.

**If you own DOOM** (for example from Steam or GOG), you can play the real
game instead: see [Playing your own copy of DOOM](#playing-your-own-copy-of-doom).

## Step 1. Open Terminal

Press **Cmd + Space**, type **Terminal**, and press **Return**. A window
with a line of text and a blinking cursor opens. This is where you will
paste the commands.

To paste a command: copy it from this page (select it, then **Cmd + C**),
click in the Terminal window, press **Cmd + V**, then press **Return**.

## Step 2. Install the four tools

The game script checks for these itself, and if one is missing it stops
and tells you what to type. You can install them all now instead.

### 2a. Apple's Command Line Tools

These are Apple's free developer tools (the C toolchain and `git`). Paste:

```sh
xcode-select --install
```

A window asks whether to install the tools: click **Install**, then
**Agree**. It takes about 5 to 10 minutes. If Terminal says
`command line tools are already installed`, you already have them.

### 2b. Homebrew

Homebrew installs free software on a Mac. Paste this line (it is long;
copy all of it):

```sh
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

It asks for your Mac's password. As you type the password **nothing
appears on screen**; that is normal. Press Return when done, and press
Return again when it asks you to confirm.

When it finishes, it prints **Next steps** with two or three commands that
start with `echo` and `eval`. Copy and paste those too: they let Terminal
find Homebrew from now on. (If you skip them, the game script still finds
Homebrew, but the `brew` command in step 2d will say
`command not found`; use `/opt/homebrew/bin/brew install sdl2` there.)

### 2c. Rust

The compiler is written in a language called Rust. Paste:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

When it asks how to proceed, press **Return** to choose the default.
When it says `Rust is installed now`, **quit Terminal (Cmd + Q) and open
it again** so it can find Rust.

### 2d. SDL2

SDL2 is the library that opens the game window and reads the keyboard.
Paste:

```sh
brew install sdl2
```

(Homebrew now installs SDL2 as `sdl2-compat`, which runs on top of SDL3.
That is expected.)

## Step 3. Get this project

The project's code is on GitHub, in a private repository, so download it
while signed in to GitHub:

1. In Safari, go to <https://github.com/spacefilm900/c-compiler> and sign
   in if asked.
2. Click the green **Code** button, then **Download ZIP**.
3. Safari unpacks it into your **Downloads** folder as a folder called
   `c-compiler-main`.

In Terminal, go into that folder:

```sh
cd ~/Downloads/c-compiler-main
```

If macOS asks whether **Terminal may access files in your Downloads
folder**, click **Allow**.

(If you use `git`: `git clone https://github.com/spacefilm900/c-compiler.git`
and `cd c-compiler` work just as well.)

## Step 4. Build and play

Paste:

```sh
./tools/mac/play_doom.sh
```

What you will see:

1. `==> Checking what this Mac needs` and then
   `Everything needed is installed.` If something is missing, it says
   what, and the exact line to paste; do that, then run the command again.
2. `==> Building the C compiler`. The first time, Rust downloads the exact
   version this project uses (lines starting `info:`), then builds. This
   takes a minute or two.
3. `==> Fetching the DOOM source code` and the Freedoom game data.
4. `==> Compiling DOOM with this project's compiler`. This compiles DOOM's 80
   C files and the window code. Under a minute.
5. `==> Linking`, then `==> Starting DOOM with freedoom1.wad`.
6. A window titled **DOOM** opens with the title screen. If you don't see
   it, it may be behind Terminal: click its icon in the Dock, or press
   **Cmd + Tab**.

Next time, open Terminal and paste just these two lines; the game starts
in a few seconds because nothing needs building again:

```sh
cd ~/Downloads/c-compiler-main
./tools/mac/play_doom.sh
```

## Controls

| Key | What it does |
|---|---|
| Up / Down arrows (or W / S) | Walk forwards / backwards |
| Left / Right arrows | Turn |
| A / D | Step sideways (strafe) |
| **control** (bottom left of the keyboard) | Fire |
| Space (or E) | Open doors, press switches |
| shift (hold) | Run |
| option (hold) with Left / Right | Step sideways |
| 1 to 7 | Choose a weapon |
| tab | Show the map (tab again to close) |
| esc | Menu (arrows to choose, Return to select, esc to go back) |
| Y / N | Answer yes / no questions in the menu |
| - and = | Make the view smaller / bigger |
| fn + F2 / fn + F3 | Save / load a game |

The mouse does nothing; DOOM is played with the keyboard. There is no
sound in this version.

**To start playing:** press **esc** (or Return) at the title screen,
choose **New Game**, press Return, pick an episode and a difficulty with
the arrows and Return.

**To quit:** press **esc**, choose **Quit Game**, and press **Y**. You can
also press **Cmd + Q** or close the window.

The window can be resized by dragging its corner; the picture keeps DOOM's
shape. Settings and saved games are kept in
`build/doom-sdl/saves` inside the project folder.

## Playing your own copy of DOOM

If you own DOOM, find its game data file: `DOOM.WAD` (The Ultimate DOOM),
`DOOM2.WAD` (DOOM II), or the shareware `DOOM1.WAD`. On Steam it is inside
the game's install folder; on GOG, in the game's folder.

Then type `./tools/mac/play_doom.sh --wad ` (with a space at the end),
**drag the WAD file from Finder into the Terminal window** (this types its
location for you), and press Return. It looks like this:

```sh
./tools/mac/play_doom.sh --wad /Users/keith/Documents/DOOM.WAD
```

The script also downloads Freedoom: Phase 2, which plays like DOOM II:

```sh
./tools/mac/play_doom.sh --wad tests/external/freedoom/freedoom2.wad
```

Anything else you add goes to the game. For example, to go straight to
episode 1, level 1 on the middle difficulty: `./tools/mac/play_doom.sh -warp 1 1 -skill 3`.

## What was built with what

- **This project's compiler compiles every C file:** all 80 of DOOM's
  (doomgeneric's engine, unchanged) and the window and keyboard code,
  `harness/doom/dg_sdl.c`, including SDL2's headers.
- The compiler's output is LLVM IR, a text form of machine instructions.
  **Apple's Clang** turns that IR into ARM machine code and **links** the
  game with the SDL2 library and macOS's C library. It never sees DOOM's C
  source. This is how the project always builds programs (see the README).
- SDL2 itself is Homebrew's ready-built library, not compiled here.

## If something goes wrong

**`zsh: permission denied: ./tools/mac/play_doom.sh`**
The download lost the file's "may run" mark. Use this instead:

```sh
bash tools/mac/play_doom.sh
```

**`zsh: no such file or directory: ./tools/mac/play_doom.sh`** or
**`cd: no such file or directory`**
Terminal is not in the project folder, or the folder has another name.
Type `cd ` (with a space), drag the project folder from Finder into the
Terminal window, press Return, and try again.

**"cannot be opened because the developer cannot be verified"**, or
**"Apple could not verify ... is free of malware"**
The game and compiler are built on your Mac, so macOS should not ask. If
it does (usually because the ZIP was downloaded with a browser), clear
the download mark from the project folder and try again:

```sh
xattr -dr com.apple.quarantine ~/Downloads/c-compiler-main
```

Or open **System Settings > Privacy & Security**, scroll down, and click
**Open Anyway** next to the message.

**`MISSING: ...`**
The script found a tool missing. Paste the line it shows, then run
`./tools/mac/play_doom.sh` again.

**`brew: command not found` or `cargo: command not found`**
Quit Terminal and open it again. For Homebrew, run the "Next steps"
commands from step 2b, or paste:

```sh
echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
eval "$(/opt/homebrew/bin/brew shellenv)"
```

On Apple Silicon Macs Homebrew lives in `/opt/homebrew` (on older Intel
Macs it was `/usr/local`); the script looks in both.

**`xcrun: error: invalid active developer path`**
The Command Line Tools are missing or were removed by a macOS update. Run
`xcode-select --install` again.

**The window does not appear**
Look for it with **Cmd + Tab** or in the Dock. If Terminal shows
`DOOM: cannot open a window`, copy the message for a report.

**The keys do nothing**
Click inside the DOOM window first, so it has the keyboard rather than
Terminal. The function keys need **fn** on a MacBook keyboard.

**Something else**
Copy everything Terminal printed (Cmd + A, Cmd + C) and send it to whoever
maintains this project. The line `This compiler could not compile ...`
means a bug in this project's compiler, which is useful to know.

## For developers

- `./tools/mac/play_doom.sh --build-only` builds without starting.
  `DOOM_OPT=O0` builds the IR unoptimised. The game binary is
  `build/doom-sdl/doom`.
- Test settings in `harness/doom/dg_sdl.c`: `DOOM_EXIT_AFTER_MS`,
  `DOOM_SCREENSHOT` (the last frame as a PPM picture) and `DOOM_TEST_KEYS`
  (scripted key presses). With `SDL_VIDEODRIVER=dummy` the game runs with
  no screen.
- The same script works on Linux with SDL2's development files
  (`sdl2-config`), or with `SDL_CFLAGS` and `SDL_LIBS` set.
- The `macos` workflow's manual DOOM job
  (`gh workflow run macos.yml -f doom=true`) compares DOOM's frame hashes
  between this compiler's build and Apple Clang's at `-O0` and `-O2`,
  every Freedoom demo to its end, and runs this script on the Mac with
  the dummy driver and scripted keys. See STATUS.md for its results.

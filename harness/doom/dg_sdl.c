/*
 * A doomgeneric display backend for SDL2: a window, the keyboard, and the
 * real clock. This file is part of this repository (MIT licence). It
 * follows the shape of doomgeneric's own doomgeneric_sdl.c (the DG_*
 * functions doomgeneric.h declares, plus main), written for this project
 * so that it compiles as strict C with this compiler and runs on macOS.
 *
 * - The window is resizable and starts at three times DOOM's 320x200
 *   (doomgeneric renders 640x400); the picture keeps its 4:3 shape.
 * - Keys follow DOOM's defaults: arrows move and turn, Ctrl fires, Space
 *   opens doors, Shift runs, Alt strafes (hold it and turn), Escape opens the menu, Enter
 *   selects. W/A/S/D also move and strafe, and E also opens doors.
 * - No sound: doomgeneric's sound needs SDL_mixer, and this build leaves
 *   FEATURE_SOUND off.
 * - DOOM_EXIT_AFTER_MS=N makes the game quit by itself after N
 *   milliseconds; CI uses it with SDL_VIDEODRIVER=dummy to show that the
 *   game starts and draws frames without a screen.
 */

#define SDL_MAIN_HANDLED
#include <SDL.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "doomgeneric.h"
#include "doomkeys.h"

static SDL_Window *window;
static SDL_Renderer *renderer;
static SDL_Texture *texture;
static uint32_t exit_after_ms;
static unsigned long frames_drawn;

#define KEYQUEUE_SIZE 64

static unsigned short key_queue[KEYQUEUE_SIZE];
static unsigned int key_write;
static unsigned int key_read;

static unsigned char doom_key(SDL_Keycode key)
{
    switch (key) {
    case SDLK_RETURN:
    case SDLK_KP_ENTER:
        return KEY_ENTER;
    case SDLK_ESCAPE:
        return KEY_ESCAPE;
    case SDLK_LEFT:
        return KEY_LEFTARROW;
    case SDLK_RIGHT:
        return KEY_RIGHTARROW;
    case SDLK_UP:
    case SDLK_w:
        return KEY_UPARROW;
    case SDLK_DOWN:
    case SDLK_s:
        return KEY_DOWNARROW;
    case SDLK_a:
        return KEY_STRAFE_L;
    case SDLK_d:
        return KEY_STRAFE_R;
    case SDLK_LCTRL:
    case SDLK_RCTRL:
        return KEY_FIRE;
    case SDLK_SPACE:
    case SDLK_e:
        return KEY_USE;
    case SDLK_LSHIFT:
    case SDLK_RSHIFT:
        return KEY_RSHIFT;
    case SDLK_LALT:
    case SDLK_RALT:
        return KEY_RALT;
    case SDLK_TAB:
        return KEY_TAB;
    case SDLK_BACKSPACE:
        return KEY_BACKSPACE;
    case SDLK_PAUSE:
        return KEY_PAUSE;
    case SDLK_F1:
        return KEY_F1;
    case SDLK_F2:
        return KEY_F2;
    case SDLK_F3:
        return KEY_F3;
    case SDLK_F4:
        return KEY_F4;
    case SDLK_F5:
        return KEY_F5;
    case SDLK_F6:
        return KEY_F6;
    case SDLK_F7:
        return KEY_F7;
    case SDLK_F8:
        return KEY_F8;
    case SDLK_F9:
        return KEY_F9;
    case SDLK_F10:
        return KEY_F10;
    case SDLK_F11:
        return KEY_F11;
    case SDLK_EQUALS:
    case SDLK_PLUS:
        return KEY_EQUALS;
    case SDLK_MINUS:
        return KEY_MINUS;
    default:
        break;
    }
    /* Printable keys: letters arrive as lower case, digits and y/n for
       the menus as themselves. */
    if (key >= 'A' && key <= 'Z')
        return (unsigned char)(key - 'A' + 'a');
    if (key > 0 && key < 128)
        return (unsigned char)key;
    return 0;
}

static void queue_key(int pressed, SDL_Keycode code)
{
    unsigned char key = doom_key(code);

    if (key == 0)
        return;
    key_queue[key_write] = (unsigned short)((pressed << 8) | key);
    key_write = (key_write + 1) % KEYQUEUE_SIZE;
}

static void poll_events(void)
{
    SDL_Event e;

    while (SDL_PollEvent(&e)) {
        if (e.type == SDL_QUIT) {
            SDL_Quit();
            exit(0);
        }
        if (e.type == SDL_KEYDOWN && !e.key.repeat)
            queue_key(1, e.key.keysym.sym);
        else if (e.type == SDL_KEYUP)
            queue_key(0, e.key.keysym.sym);
    }
}

void DG_Init(void)
{
    const char *limit = getenv("DOOM_EXIT_AFTER_MS");

    if (limit != NULL)
        exit_after_ms = (uint32_t)strtoul(limit, NULL, 10);
    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER | SDL_INIT_EVENTS) != 0) {
        fprintf(stderr, "DOOM: SDL could not start: %s\n", SDL_GetError());
        exit(1);
    }
    window = SDL_CreateWindow("DOOM", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
                              960, 720, SDL_WINDOW_SHOWN | SDL_WINDOW_RESIZABLE |
                              SDL_WINDOW_ALLOW_HIGHDPI);
    if (window == NULL) {
        fprintf(stderr, "DOOM: cannot open a window: %s\n", SDL_GetError());
        exit(1);
    }
    renderer = SDL_CreateRenderer(window, -1, 0);
    if (renderer == NULL) {
        fprintf(stderr, "DOOM: cannot draw in the window: %s\n", SDL_GetError());
        exit(1);
    }
    /* DOOM's pixels were not square: 320x200 filled a 4:3 screen. */
    SDL_RenderSetLogicalSize(renderer, 640, 480);
    texture = SDL_CreateTexture(renderer, SDL_PIXELFORMAT_ARGB8888,
                                SDL_TEXTUREACCESS_STREAMING, DOOMGENERIC_RESX, DOOMGENERIC_RESY);
    if (texture == NULL) {
        fprintf(stderr, "DOOM: cannot create the picture: %s\n", SDL_GetError());
        exit(1);
    }
    SDL_SetRenderDrawColor(renderer, 0, 0, 0, 255);
    SDL_RenderClear(renderer);
    SDL_RenderPresent(renderer);
}

void DG_DrawFrame(void)
{
    SDL_UpdateTexture(texture, NULL, DG_ScreenBuffer, DOOMGENERIC_RESX * (int)sizeof(uint32_t));
    SDL_RenderClear(renderer);
    SDL_RenderCopy(renderer, texture, NULL, NULL);
    SDL_RenderPresent(renderer);
    frames_drawn++;
    poll_events();
    if (exit_after_ms != 0 && SDL_GetTicks() >= exit_after_ms) {
        printf("DOOM: drew %lu frames, exiting after %u ms as asked\n", frames_drawn,
               (unsigned)exit_after_ms);
        SDL_Quit();
        exit(0);
    }
}

void DG_SleepMs(uint32_t ms)
{
    SDL_Delay(ms);
}

uint32_t DG_GetTicksMs(void)
{
    return SDL_GetTicks();
}

int DG_GetKey(int *pressed, unsigned char *key)
{
    unsigned short data;

    if (key_read == key_write)
        return 0;
    data = key_queue[key_read];
    key_read = (key_read + 1) % KEYQUEUE_SIZE;
    *pressed = data >> 8;
    *key = data & 0xFF;
    return 1;
}

void DG_SetWindowTitle(const char *title)
{
    if (window != NULL)
        SDL_SetWindowTitle(window, title);
}

int main(int argc, char **argv)
{
    SDL_SetMainReady();
    doomgeneric_Create(argc, argv);
    for (;;)
        doomgeneric_Tick();
}

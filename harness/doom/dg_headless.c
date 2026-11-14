/*
 * Headless doomgeneric backend for the DOOM differential harness.
 * This file is part of this repository (MIT licence). It is not taken
 * from doomgeneric; it only implements the six DG_* hooks that
 * doomgeneric.h declares, plus main().
 *
 * - No window and no input: the game is driven by a recorded demo given
 *   on the command line (for example -timedemo demo1).
 * - A fake clock: DG_GetTicksMs advances by one millisecond per call and
 *   DG_SleepMs advances it by the requested amount. Nothing reads the
 *   real time, so a run is fully deterministic.
 * - Every frame handed to DG_DrawFrame is hashed (a 64-bit FNV-1a style
 *   hash that mixes one 32-bit pixel of DG_ScreenBuffer per step) and
 *   written to the log as
 *   "<frame> <hash>". After --frames frames the log gets a final
 *   "total <frames> <hash-of-hashes>" line and the program exits 0.
 * - When the demo ends first, -timedemo reports "timed N gametics" through
 *   I_Error, which exits with status 255. An atexit handler writes the
 *   same "total" line then, so a demo played to its natural end is
 *   compared in full.
 *
 * Harness options are removed from argv before doomgeneric sees it:
 *   --frames N   stop after N frames (default 2100)
 *   --log PATH   frame-hash log (default: standard output)
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "doomgeneric.h"

static uint32_t fake_now_ms;
static long frames_wanted = 2100;
static long frames_done;
static FILE *hash_log;
static uint64_t total_hash = 0xcbf29ce484222325u;
static int total_written;

static void write_total(void)
{
    if (total_written || hash_log == NULL)
        return;
    total_written = 1;
    fprintf(hash_log, "total %ld %016llx\n", frames_done,
            (unsigned long long)total_hash);
    fflush(hash_log);
    if (hash_log != stdout)
        fclose(hash_log);
}

static uint64_t fnv1a_step(uint64_t h, uint32_t word)
{
    h ^= word;
    h *= 0x100000001b3u;
    return h;
}

void DG_Init(void)
{
}

void DG_DrawFrame(void)
{
    uint64_t h = 0xcbf29ce484222325u;
    long n = (long)DOOMGENERIC_RESX * DOOMGENERIC_RESY;
    long i;

    for (i = 0; i < n; i++)
        h = fnv1a_step(h, (uint32_t)DG_ScreenBuffer[i]);
    frames_done++;
    total_hash = fnv1a_step(total_hash, (uint32_t)h);
    total_hash = fnv1a_step(total_hash, (uint32_t)(h >> 32));
    fprintf(hash_log, "%ld %016llx\n", frames_done, (unsigned long long)h);
    if (frames_done >= frames_wanted) {
        write_total();
        exit(0);
    }
}

void DG_SleepMs(uint32_t ms)
{
    fake_now_ms += ms;
}

uint32_t DG_GetTicksMs(void)
{
    fake_now_ms += 1;
    return fake_now_ms;
}

int DG_GetKey(int *pressed, unsigned char *key)
{
    (void)pressed;
    (void)key;
    return 0;
}

void DG_SetWindowTitle(const char *title)
{
    (void)title;
}

int main(int argc, char **argv)
{
    char **rest = malloc(sizeof(char *) * (size_t)(argc + 1));
    const char *log_path = NULL;
    int n = 0;
    int i;

    if (rest == NULL)
        return 2;
    for (i = 0; i < argc; i++) {
        if (i > 0 && strcmp(argv[i], "--frames") == 0 && i + 1 < argc) {
            frames_wanted = strtol(argv[++i], NULL, 10);
        } else if (i > 0 && strcmp(argv[i], "--log") == 0 && i + 1 < argc) {
            log_path = argv[++i];
        } else {
            rest[n++] = argv[i];
        }
    }
    rest[n] = NULL;
    if (frames_wanted < 1)
        frames_wanted = 1;

    hash_log = stdout;
    if (log_path != NULL) {
        hash_log = fopen(log_path, "w");
        if (hash_log == NULL) {
            perror(log_path);
            return 2;
        }
    }

    atexit(write_total);
    doomgeneric_Create(n, rest);
    for (;;)
        doomgeneric_Tick();
}

/* Minimal headless host. Build locally against the upstream libretro header. */
#include <libretro.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned buttons, width, height, video_frames, audio_frames;
static size_t pitch;
static const void *pixels;
static double audio_energy;
static bool inspect_audio;

static bool environment(unsigned command, void *data)
{
    switch (command) {
    case RETRO_ENVIRONMENT_SET_PIXEL_FORMAT:
        return *(const enum retro_pixel_format *)data == RETRO_PIXEL_FORMAT_RGB565;
    case RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE:
        *(int *)data = 3;
        return true;
    case RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE:
        *(bool *)data = false;
        return true;
    case RETRO_ENVIRONMENT_GET_VARIABLE: {
        struct retro_variable *option = data;
        if (strcmp(option->key, "snes9x_overscan") == 0) {
            option->value = "enabled"; /* Crop to the 224-line picture. */
            return true;
        }
        return false;
    }
    default:
        return false; /* Keep the core defaults for every other option. */
    }
}

static void video(const void *data, unsigned w, unsigned h, size_t stride)
{
    if (data) {
        pixels = data;
        width = w;
        height = h;
        pitch = stride;
        ++video_frames;
    }
}

static size_t audio(const int16_t *data, size_t frames)
{
    audio_frames += frames;
    if (inspect_audio)
        for (size_t i = 0; i < frames * 2; ++i) {
            double sample = data[i] / 32768.0;
            audio_energy += sample * sample;
        }
    return frames;
}

static void audio_sample(int16_t left, int16_t right)
{
    const int16_t pair[] = {left, right};
    audio(pair, 1);
}

static void poll(void) {}

static int16_t input(unsigned port, unsigned device, unsigned index, unsigned id)
{
    if (port != 0 || device != RETRO_DEVICE_JOYPAD || index != 0)
        return 0;
    /* The first twelve libretro SNES buttons are ordered B through R. */
    return id < 12 ? (buttons >> (15 - id)) & 1 : 0;
}

int bench_load(const void *data, size_t size)
{
    const struct retro_game_info game = {.path = "benchmark.sfc", .data = data, .size = size};
    retro_set_environment(environment);
    retro_set_video_refresh(video);
    retro_set_audio_sample(audio_sample);
    retro_set_audio_sample_batch(audio);
    retro_set_input_poll(poll);
    retro_set_input_state(input);
    retro_init();
    return retro_load_game(&game) && retro_get_region() == RETRO_REGION_NTSC;
}

void bench_unload(void)
{
    retro_unload_game();
    retro_deinit();
}

void bench_frame(unsigned mask, unsigned checkpoint)
{
    buttons = mask;
    inspect_audio = checkpoint != 0;
    audio_energy = 0;
    retro_run();
}

unsigned bench_video_frames(void) { return video_frames; }
unsigned bench_audio_frames(void) { return audio_frames; }
unsigned bench_width(void) { return width; }
unsigned bench_height(void) { return height; }
double bench_audio_energy(void) { return audio_energy; }

unsigned bench_video(void)
{
    unsigned hash = 2166136261u;
    const unsigned char *row = pixels;
    for (unsigned y = 0; y < height; ++y, row += pitch)
        for (unsigned x = 0; x < width * 2; ++x)
            hash = (hash ^ row[x]) * 16777619u;
    return hash;
}

unsigned bench_wram(void)
{
    unsigned hash = 2166136261u;
    const unsigned char *ram = retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM);
    size_t size = retro_get_memory_size(RETRO_MEMORY_SYSTEM_RAM);
    for (size_t i = 0; i < size; ++i)
        hash = (hash ^ ram[i]) * 16777619u;
    return hash;
}

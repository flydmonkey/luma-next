#include <obs.h>
#include <windows.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct luma_obs {
    obs_scene_t *scene;
    obs_source_t *monitor;
    obs_source_t *desktop_audio;
    obs_output_t *output;
    obs_encoder_t *video_encoder;
    obs_encoder_t *audio_encoder;
};

struct primary_display {
    char id[128];
};

static BOOL CALLBACK find_primary_display(HMONITOR handle, HDC dc, LPRECT rect, LPARAM parameter)
{
    (void)dc;
    (void)rect;
    struct primary_display *result = (struct primary_display *)parameter;
    MONITORINFOEXA monitor = {0};
    monitor.cbSize = sizeof(monitor);
    if (!GetMonitorInfoA(handle, (LPMONITORINFO)&monitor) || !(monitor.dwFlags & MONITORINFOF_PRIMARY))
        return TRUE;
    DISPLAY_DEVICEA device = {0};
    device.cb = sizeof(device);
    if (EnumDisplayDevicesA(monitor.szDevice, 0, &device, EDD_GET_DEVICE_INTERFACE_NAME))
        snprintf(result->id, sizeof(result->id), "%s", device.DeviceID);
    else
        snprintf(result->id, sizeof(result->id), "%s", monitor.szDevice);
    return FALSE;
}

static void set_error(char *error, size_t size, const char *message)
{
    if (error && size) {
        snprintf(error, size, "%s", message ? message : "unknown libobs error");
    }
}

static bool load_module(const char *root, const char *name, char *error, size_t error_size)
{
    char binary[MAX_PATH * 2];
    char data[MAX_PATH * 2];
    obs_module_t *module = NULL;
    snprintf(binary, sizeof(binary), "%s/obs-plugins/64bit/%s.dll", root, name);
    snprintf(data, sizeof(data), "%s/data/obs-plugins/%s", root, name);
    int result = obs_open_module(&module, binary, data);
    if (result != MODULE_SUCCESS) {
        char message[512];
        snprintf(message, sizeof(message), "failed to open OBS module %s (code %d)", name, result);
        set_error(error, error_size, message);
        return false;
    }
    if (!obs_init_module(module)) {
        char message[512];
        snprintf(message, sizeof(message), "failed to initialize OBS module %s", name);
        set_error(error, error_size, message);
        return false;
    }
    return true;
}

static void release_recording(struct luma_obs *ctx)
{
    if (ctx->output) {
        obs_output_release(ctx->output);
        ctx->output = NULL;
    }
    if (ctx->video_encoder) {
        obs_encoder_release(ctx->video_encoder);
        ctx->video_encoder = NULL;
    }
    if (ctx->audio_encoder) {
        obs_encoder_release(ctx->audio_encoder);
        ctx->audio_encoder = NULL;
    }
}

struct luma_obs *luma_obs_initialize(const char *root, const char *config_path, char *error, size_t error_size)
{
    struct luma_obs *ctx = calloc(1, sizeof(*ctx));
    if (!ctx) {
        set_error(error, error_size, "out of memory");
        return NULL;
    }
    if (!obs_startup("en-US", config_path, NULL)) {
        set_error(error, error_size, "obs_startup failed");
        free(ctx);
        return NULL;
    }
    char libobs_data[MAX_PATH * 2];
    snprintf(libobs_data, sizeof(libobs_data), "%s/data/libobs/", root);
    obs_add_data_path(libobs_data);

    uint32_t base_width = (uint32_t)GetSystemMetrics(SM_CXSCREEN);
    uint32_t base_height = (uint32_t)GetSystemMetrics(SM_CYSCREEN);
    if (!base_width || !base_height) {
        set_error(error, error_size, "could not determine the primary display size");
        goto fail;
    }
    uint32_t output_width = base_width;
    uint32_t output_height = base_height;
    if (output_width > 1920 || output_height > 1080) {
        double scale_x = 1920.0 / (double)output_width;
        double scale_y = 1080.0 / (double)output_height;
        double scale = scale_x < scale_y ? scale_x : scale_y;
        output_width = ((uint32_t)(output_width * scale)) & ~1U;
        output_height = ((uint32_t)(output_height * scale)) & ~1U;
    }

    struct obs_video_info video = {0};
    video.graphics_module = "libobs-d3d11.dll";
    video.fps_num = 30;
    video.fps_den = 1;
    video.base_width = base_width;
    video.base_height = base_height;
    video.output_width = output_width;
    video.output_height = output_height;
    video.output_format = VIDEO_FORMAT_NV12;
    video.adapter = 0;
    video.gpu_conversion = true;
    video.colorspace = VIDEO_CS_709;
    video.range = VIDEO_RANGE_PARTIAL;
    video.scale_type = OBS_SCALE_BICUBIC;
    int video_result = obs_reset_video(&video);
    if (video_result != OBS_VIDEO_SUCCESS) {
        char message[256];
        snprintf(message, sizeof(message), "obs_reset_video failed (code %d)", video_result);
        set_error(error, error_size, message);
        goto fail;
    }

    struct obs_audio_info audio = {0};
    audio.samples_per_sec = 48000;
    audio.speakers = SPEAKERS_STEREO;
    if (!obs_reset_audio(&audio)) {
        set_error(error, error_size, "obs_reset_audio failed");
        goto fail;
    }

    if (!load_module(root, "win-capture", error, error_size) ||
        !load_module(root, "win-wasapi", error, error_size) ||
        !load_module(root, "obs-x264", error, error_size) ||
        !load_module(root, "obs-ffmpeg", error, error_size)) {
        goto fail;
    }
    obs_post_load_modules();

    obs_data_t *monitor_settings = obs_data_create();
    struct primary_display primary = {0};
    EnumDisplayMonitors(NULL, NULL, find_primary_display, (LPARAM)&primary);
    if (!primary.id[0]) {
        set_error(error, error_size, "failed to identify the primary display");
        obs_data_release(monitor_settings);
        goto fail;
    }
    // WGC is the reliable path on this Windows 11 host; DXGI DuplicateOutput1 returns
    // DXGI_ERROR_UNSUPPORTED on the available AMD adapter.
    obs_data_set_int(monitor_settings, "method", 2);
    obs_data_set_string(monitor_settings, "monitor_id", primary.id);
    obs_data_set_bool(monitor_settings, "capture_cursor", true);
    ctx->monitor = obs_source_create("monitor_capture", "Luma display", monitor_settings, NULL);
    obs_data_release(monitor_settings);
    if (!ctx->monitor) {
        set_error(error, error_size, "failed to create monitor_capture source");
        goto fail;
    }

    obs_data_t *audio_settings = obs_data_create();
    obs_data_set_string(audio_settings, "device_id", "default");
    ctx->desktop_audio = obs_source_create("wasapi_output_capture", "Luma desktop audio", audio_settings, NULL);
    obs_data_release(audio_settings);
    if (!ctx->desktop_audio) {
        set_error(error, error_size, "failed to create wasapi_output_capture source");
        goto fail;
    }

    ctx->scene = obs_scene_create("Luma fullscreen");
    if (!ctx->scene || !obs_scene_add(ctx->scene, ctx->monitor)) {
        set_error(error, error_size, "failed to create the fullscreen scene");
        goto fail;
    }
    obs_set_output_source(0, obs_scene_get_source(ctx->scene));
    obs_set_output_source(1, ctx->desktop_audio);
    return ctx;

fail:
    if (ctx->scene) obs_scene_release(ctx->scene);
    if (ctx->monitor) obs_source_release(ctx->monitor);
    if (ctx->desktop_audio) obs_source_release(ctx->desktop_audio);
    obs_shutdown();
    free(ctx);
    return NULL;
}

bool luma_obs_start(struct luma_obs *ctx, const char *path, char *error, size_t error_size)
{
    if (!ctx || ctx->output) {
        set_error(error, error_size, "recording is already active");
        return false;
    }

    obs_data_t *video_settings = obs_data_create();
    obs_data_set_string(video_settings, "rate_control", "CBR");
    obs_data_set_int(video_settings, "bitrate", 6000);
    obs_data_set_int(video_settings, "keyint_sec", 2);
    obs_data_set_string(video_settings, "preset", "veryfast");
    obs_data_set_string(video_settings, "profile", "high");
    ctx->video_encoder = obs_video_encoder_create("obs_x264", "Luma H.264", video_settings, NULL);
    obs_data_release(video_settings);

    obs_data_t *audio_settings = obs_data_create();
    obs_data_set_int(audio_settings, "bitrate", 160);
    ctx->audio_encoder = obs_audio_encoder_create("ffmpeg_aac", "Luma AAC", audio_settings, 0, NULL);
    obs_data_release(audio_settings);

    obs_data_t *output_settings = obs_data_create();
    obs_data_set_string(output_settings, "path", path);
    ctx->output = obs_output_create("ffmpeg_muxer", "Luma recording", output_settings, NULL);
    obs_data_release(output_settings);

    if (!ctx->video_encoder || !ctx->audio_encoder || !ctx->output) {
        set_error(error, error_size, "failed to create OBS output or encoders");
        release_recording(ctx);
        return false;
    }

    obs_encoder_set_video(ctx->video_encoder, obs_get_video());
    obs_encoder_set_audio(ctx->audio_encoder, obs_get_audio());
    obs_output_set_video_encoder(ctx->output, ctx->video_encoder);
    obs_output_set_audio_encoder(ctx->output, ctx->audio_encoder, 0);

    if (!obs_output_start(ctx->output)) {
        set_error(error, error_size, obs_output_get_last_error(ctx->output));
        release_recording(ctx);
        return false;
    }
    Sleep(750);
    if (!obs_output_active(ctx->output)) {
        const char *last_error = obs_output_get_last_error(ctx->output);
        set_error(error, error_size, last_error && *last_error ? last_error : "OBS output stopped during startup");
        release_recording(ctx);
        return false;
    }
    return true;
}

bool luma_obs_stop(struct luma_obs *ctx, uint32_t *encoded_frames, uint64_t *total_bytes,
                   char *error, size_t error_size)
{
    if (!ctx || !ctx->output) {
        set_error(error, error_size, "no recording is active");
        return false;
    }
    obs_output_stop(ctx->output);
    for (int i = 0; i < 150 && obs_output_active(ctx->output); ++i) {
        Sleep(100);
    }
    if (obs_output_active(ctx->output)) {
        obs_output_force_stop(ctx->output);
        set_error(error, error_size, "OBS output did not stop within 15 seconds");
        return false;
    }
    Sleep(500);
    if (encoded_frames) *encoded_frames = obs_encoder_get_encoded_frames(ctx->video_encoder);
    if (total_bytes) *total_bytes = obs_output_get_total_bytes(ctx->output);
    const char *last_error = obs_output_get_last_error(ctx->output);
    if (last_error && *last_error) {
        set_error(error, error_size, last_error);
        release_recording(ctx);
        return false;
    }
    release_recording(ctx);
    return true;
}

void luma_obs_shutdown(struct luma_obs *ctx)
{
    if (!ctx) return;
    if (ctx->output) {
        obs_output_force_stop(ctx->output);
        release_recording(ctx);
    }
    obs_set_output_source(0, NULL);
    obs_set_output_source(1, NULL);
    if (ctx->scene) obs_scene_release(ctx->scene);
    if (ctx->monitor) obs_source_release(ctx->monitor);
    if (ctx->desktop_audio) obs_source_release(ctx->desktop_audio);
    obs_shutdown();
    free(ctx);
}

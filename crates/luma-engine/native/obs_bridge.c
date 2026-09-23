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
    obs_source_t *window;
    obs_source_t *game;
    obs_source_t *audio_visual;
    obs_source_t *desktop_audio;
    obs_source_t *microphone;
    obs_sceneitem_t *visual_item;
    obs_output_t *output;
    obs_encoder_t *video_encoder;
    obs_encoder_t *audio_encoder;
    ULONGLONG recording_started_ms;
    ULONGLONG pause_started_ms;
    ULONGLONG paused_total_ms;
    char qsv_module_error[512];
};

struct display_list {
    char *buffer;
    size_t size;
    size_t written;
};

static RECT physical_monitor_rect(HMONITOR handle, const RECT *fallback)
{
    MONITORINFOEXA monitor = {0};
    monitor.cbSize = sizeof(monitor);
    DEVMODEA mode = {0};
    mode.dmSize = sizeof(mode);
    if (GetMonitorInfoA(handle, (LPMONITORINFO)&monitor) &&
        EnumDisplaySettingsExA(monitor.szDevice, ENUM_CURRENT_SETTINGS, &mode, EDS_RAWMODE)) {
        RECT physical = {
            mode.dmPosition.x,
            mode.dmPosition.y,
            mode.dmPosition.x + (LONG)mode.dmPelsWidth,
            mode.dmPosition.y + (LONG)mode.dmPelsHeight,
        };
        return physical;
    }
    return *fallback;
}

static BOOL CALLBACK append_display(HMONITOR handle, HDC dc, LPRECT rect, LPARAM parameter)
{
    (void)dc;
    struct display_list *list = (struct display_list *)parameter;
    MONITORINFOEXA monitor = {0};
    monitor.cbSize = sizeof(monitor);
    if (!GetMonitorInfoA(handle, (LPMONITORINFO)&monitor)) return TRUE;
    DISPLAY_DEVICEA device = {0};
    device.cb = sizeof(device);
    const char *id = monitor.szDevice;
    const char *name = monitor.szDevice;
    if (EnumDisplayDevicesA(monitor.szDevice, 0, &device, EDD_GET_DEVICE_INTERFACE_NAME)) {
        if (device.DeviceID[0]) id = device.DeviceID;
        if (device.DeviceString[0]) name = device.DeviceString;
    }
    RECT physical = physical_monitor_rect(handle, rect);
    int result = snprintf(list->buffer + list->written, list->size - list->written,
                          "%s\t%s\t%ld\t%ld\t%ld\t%ld\t%d\n", id, name,
                          physical.left, physical.top, physical.right - physical.left,
                          physical.bottom - physical.top,
                          (monitor.dwFlags & MONITORINFOF_PRIMARY) != 0);
    if (result < 0 || (size_t)result >= list->size - list->written) return FALSE;
    list->written += (size_t)result;
    return TRUE;
}

size_t luma_obs_list_displays(struct luma_obs *ctx, char *buffer, size_t buffer_size)
{
    (void)ctx;
    if (!buffer || !buffer_size) return 0;
    buffer[0] = 0;
    struct display_list list = {buffer, buffer_size, 0};
    EnumDisplayMonitors(NULL, NULL, append_display, (LPARAM)&list);
    return list.written;
}

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

static void load_optional_module(const char *root, const char *name)
{
    char error[512] = {0};
    if (!load_module(root, name, error, sizeof(error)))
        blog(LOG_WARNING, "Luma optional module unavailable: %s", error);
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

static void release_dynamic_sources(struct luma_obs *ctx)
{
    obs_set_output_source(1, NULL);
    obs_set_output_source(2, NULL);
    if (ctx->microphone) {
        obs_source_release(ctx->microphone);
        ctx->microphone = NULL;
    }
    if (ctx->window || ctx->game || ctx->audio_visual) {
        if (ctx->visual_item) {
            obs_sceneitem_remove(ctx->visual_item);
            ctx->visual_item = NULL;
        }
        if (ctx->window) {
            obs_source_release(ctx->window);
            ctx->window = NULL;
        }
        if (ctx->game) {
            obs_source_release(ctx->game);
            ctx->game = NULL;
        }
        if (ctx->audio_visual) {
            obs_source_release(ctx->audio_visual);
            ctx->audio_visual = NULL;
        }
        ctx->visual_item = obs_scene_add(ctx->scene, ctx->monitor);
    }
}

struct luma_obs *luma_obs_initialize(const char *root, const char *config_path, char *error, size_t error_size)
{
    struct luma_obs *ctx = calloc(1, sizeof(*ctx));
    if (!ctx) {
        set_error(error, error_size, "out of memory");
        return NULL;
    }
    if (!SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) &&
        GetLastError() != ERROR_ACCESS_DENIED)
        blog(LOG_WARNING, "Luma could not enable Per-Monitor V2 DPI awareness (error %lu)", GetLastError());
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
    if (!load_module(root, "obs-qsv11", ctx->qsv_module_error, sizeof(ctx->qsv_module_error)))
        blog(LOG_WARNING, "Luma optional module unavailable: %s", ctx->qsv_module_error);
    load_optional_module(root, "obs-nvenc");
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
    ctx->visual_item = ctx->scene ? obs_scene_add(ctx->scene, ctx->monitor) : NULL;
    if (!ctx->scene || !ctx->visual_item) {
        set_error(error, error_size, "failed to create the fullscreen scene");
        goto fail;
    }
    obs_set_output_source(0, obs_scene_get_source(ctx->scene));
    return ctx;

fail:
    if (ctx->scene) obs_scene_release(ctx->scene);
    if (ctx->monitor) obs_source_release(ctx->monitor);
    if (ctx->desktop_audio) obs_source_release(ctx->desktop_audio);
    obs_shutdown();
    free(ctx);
    return NULL;
}

size_t luma_obs_list_property(struct luma_obs *ctx, const char *source_id, const char *property_name,
                              char *buffer, size_t buffer_size)
{
    (void)ctx;
    if (!buffer || !buffer_size) return 0;
    buffer[0] = 0;
    obs_properties_t *properties = obs_get_source_properties(source_id);
    if (!properties) return 0;
    obs_property_t *property = obs_properties_get(properties, property_name);
    size_t written = 0;
    if (property) {
        size_t count = obs_property_list_item_count(property);
        for (size_t i = 0; i < count; ++i) {
            const char *value = obs_property_list_item_string(property, i);
            const char *name = obs_property_list_item_name(property, i);
            if (!value || !*value || !name) continue;
            int result = snprintf(buffer + written, buffer_size - written, "%s\t%s\n", value, name);
            if (result < 0 || (size_t)result >= buffer_size - written) break;
            written += (size_t)result;
        }
    }
    obs_properties_destroy(properties);
    return written;
}

static bool configure_sources(struct luma_obs *ctx, const char *mode, const char *target,
                              uint32_t display_width, uint32_t display_height,
                              int32_t region_x, int32_t region_y,
                              uint32_t region_width, uint32_t region_height,
                              bool system_audio, bool microphone, const char *mic_device,
                              char *error, size_t error_size)
{
    release_dynamic_sources(ctx);
    if (strcmp(mode, "audio_only") == 0) {
        obs_data_t *settings = obs_data_create();
        obs_data_set_int(settings, "width", 32);
        obs_data_set_int(settings, "height", 32);
        obs_data_set_int(settings, "color", 0xFF000000);
        ctx->audio_visual = obs_source_create("color_source_v3", "Luma audio-only placeholder", settings, NULL);
        obs_data_release(settings);
        if (!ctx->audio_visual) {
            set_error(error, error_size, "failed to create the documented audio-only placeholder video source");
            return false;
        }
        if (ctx->visual_item) obs_sceneitem_remove(ctx->visual_item);
        ctx->visual_item = obs_scene_add(ctx->scene, ctx->audio_visual);
    } else if (strcmp(mode, "window") == 0) {
        obs_data_t *settings = obs_data_create();
        obs_data_set_string(settings, "window", target);
        obs_data_set_int(settings, "method", 2);
        obs_data_set_int(settings, "priority", 0);
        obs_data_set_bool(settings, "cursor", true);
        obs_data_set_bool(settings, "client_area", true);
        ctx->window = obs_source_create("window_capture", "Luma window", settings, NULL);
        obs_data_release(settings);
        if (!ctx->window) {
            set_error(error, error_size, "failed to create window_capture source");
            return false;
        }
        if (ctx->visual_item) obs_sceneitem_remove(ctx->visual_item);
        ctx->visual_item = obs_scene_add(ctx->scene, ctx->window);
        if (!ctx->visual_item) {
            set_error(error, error_size, "failed to add window capture to scene");
            release_dynamic_sources(ctx);
            return false;
        }
    } else if (strcmp(mode, "game") == 0) {
        obs_data_t *settings = obs_data_create();
        obs_data_set_string(settings, "capture_mode", "window");
        obs_data_set_string(settings, "window", target);
        obs_data_set_int(settings, "priority", 0);
        obs_data_set_bool(settings, "anti_cheat_hook", false);
        obs_data_set_int(settings, "hook_rate", 1);
        obs_data_set_bool(settings, "capture_cursor", false);
        ctx->game = obs_source_create("game_capture", "Luma game", settings, NULL);
        obs_data_release(settings);
        if (!ctx->game) {
            set_error(error, error_size, "failed to create OBS game_capture source; verify win-capture is installed");
            return false;
        }
        if (ctx->visual_item) obs_sceneitem_remove(ctx->visual_item);
        ctx->visual_item = obs_scene_add(ctx->scene, ctx->game);
        if (!ctx->visual_item) {
            set_error(error, error_size, "failed to add game_capture to scene");
            release_dynamic_sources(ctx);
            return false;
        }
    } else {
        obs_data_t *settings = obs_data_create();
        obs_data_set_int(settings, "method", 2);
        obs_data_set_string(settings, "monitor_id", target);
        obs_data_set_bool(settings, "capture_cursor", true);
        obs_source_update(ctx->monitor, settings);
        obs_data_release(settings);
        if (strcmp(mode, "region") == 0) {
            struct obs_sceneitem_crop crop = {0};
            crop.left = region_x;
            crop.top = region_y;
            crop.right = (int)display_width - region_x - (int)region_width;
            crop.bottom = (int)display_height - region_y - (int)region_height;
            obs_sceneitem_set_crop(ctx->visual_item, &crop);
        } else {
            struct obs_sceneitem_crop crop = {0};
            obs_sceneitem_set_crop(ctx->visual_item, &crop);
        }
    }
    if (system_audio) obs_set_output_source(1, ctx->desktop_audio);
    if (microphone) {
        obs_data_t *settings = obs_data_create();
        obs_data_set_string(settings, "device_id", mic_device && *mic_device ? mic_device : "default");
        ctx->microphone = obs_source_create("wasapi_input_capture", "Luma microphone", settings, NULL);
        obs_data_release(settings);
        if (!ctx->microphone) {
            set_error(error, error_size, "failed to create wasapi_input_capture source; check microphone privacy permission and device availability");
            release_dynamic_sources(ctx);
            return false;
        }
        obs_set_output_source(2, ctx->microphone);
    }
    return true;
}

bool luma_obs_encoder_available(struct luma_obs *ctx, const char *wanted)
{
    (void)ctx;
    const char *id = NULL;
    for (size_t index = 0; obs_enum_encoder_types(index, &id); ++index) {
        uint32_t caps = obs_get_encoder_caps(id);
        if (strcmp(id, wanted) == 0 && obs_get_encoder_type(id) == OBS_ENCODER_VIDEO &&
            strcmp(obs_get_encoder_codec(id), "h264") == 0 &&
            !(caps & (OBS_ENCODER_CAP_DEPRECATED | OBS_ENCODER_CAP_INTERNAL)))
            return true;
    }
    return false;
}

size_t luma_obs_encoder_unavailable_reason(struct luma_obs *ctx, const char *wanted,
                                           char *buffer, size_t buffer_size)
{
    if (!buffer || !buffer_size) return 0;
    buffer[0] = 0;
    if (strcmp(wanted, "obs_qsv11") == 0) {
        snprintf(buffer, buffer_size, "OBS encoder id obs_qsv11 is deprecated; use obs_qsv11_v2");
    } else if (strncmp(wanted, "obs_qsv11", 9) == 0 && ctx && ctx->qsv_module_error[0]) {
        snprintf(buffer, buffer_size, "%s", ctx->qsv_module_error);
    } else if (strncmp(wanted, "obs_qsv11", 9) == 0) {
        snprintf(buffer, buffer_size, "Intel QSV H.264 was not registered; verify an enabled Intel GPU and its media driver");
    } else {
        snprintf(buffer, buffer_size, "OBS encoder %s was not registered for H.264 on this system", wanted);
    }
    return strlen(buffer);
}

static void quality_limit(const char *quality, uint32_t *width, uint32_t *height)
{
    uint32_t limit_width = 1920;
    uint32_t limit_height = 1080;
    if (strcmp(quality, "1440p30") == 0) {
        limit_width = 2560;
        limit_height = 1440;
    } else if (strcmp(quality, "2160p30") == 0) {
        limit_width = 3840;
        limit_height = 2160;
    }
    if (*width > limit_width || *height > limit_height) {
        double scale_x = (double)limit_width / (double)*width;
        double scale_y = (double)limit_height / (double)*height;
        double scale = scale_x < scale_y ? scale_x : scale_y;
        *width = ((uint32_t)(*width * scale)) & ~1U;
        *height = ((uint32_t)(*height * scale)) & ~1U;
    }
}

static bool start_with_encoder(struct luma_obs *ctx, const char *path, const char *encoder_id, bool audio_only,
                               char *error, size_t error_size)
{
    obs_data_t *video_settings = obs_data_create();
    obs_data_set_string(video_settings, "rate_control", "CBR");
    obs_data_set_int(video_settings, "bitrate", audio_only ? 50 : 6000);
    obs_data_set_int(video_settings, "keyint_sec", 2);
    obs_data_set_string(video_settings, "preset", "veryfast");
    obs_data_set_string(video_settings, "profile", "high");
    ctx->video_encoder = obs_video_encoder_create(encoder_id, "Luma H.264", video_settings, NULL);
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
        char message[512];
        if (!ctx->video_encoder)
            snprintf(message, sizeof(message), "failed to create OBS video encoder %s; check encoder support, GPU driver, and runtime dependencies", encoder_id);
        else if (!ctx->audio_encoder)
            snprintf(message, sizeof(message), "failed to create OBS AAC audio encoder");
        else
            snprintf(message, sizeof(message), "failed to create OBS ffmpeg_muxer output");
        set_error(error, error_size, message);
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
    ctx->recording_started_ms = GetTickCount64();
    ctx->pause_started_ms = 0;
    ctx->paused_total_ms = 0;
    Sleep(750);
    if (!obs_output_active(ctx->output)) {
        const char *last_error = obs_output_get_last_error(ctx->output);
        set_error(error, error_size, last_error && *last_error ? last_error : "OBS output stopped during startup");
        release_recording(ctx);
        return false;
    }
    return true;
}

bool luma_obs_start(struct luma_obs *ctx, const char *path, const char *requested,
                    const char *mode, const char *target, const char *quality,
                    uint32_t display_width, uint32_t display_height,
                    int32_t region_x, int32_t region_y,
                    uint32_t region_width, uint32_t region_height,
                    bool system_audio, bool microphone,
                    const char *mic_device,
                    char *active, size_t active_size, char *fallback, size_t fallback_size,
                    char *error, size_t error_size)
{
    if (!ctx || ctx->output) {
        set_error(error, error_size, "recording is already active");
        return false;
    }
    bool audio_only = strcmp(mode, "audio_only") == 0;
    bool game = strcmp(mode, "game") == 0;
    uint32_t base_width = audio_only ? 32 : (game ? 1920 : (strcmp(mode, "region") == 0 ? region_width : display_width));
    uint32_t base_height = audio_only ? 32 : (game ? 1080 : (strcmp(mode, "region") == 0 ? region_height : display_height));
    if (strcmp(mode, "window") != 0 && base_width && base_height) {
        struct obs_video_info video = {0};
        video.graphics_module = "libobs-d3d11.dll";
        video.fps_num = 30;
        video.fps_den = 1;
        video.base_width = base_width & ~1U;
        video.base_height = base_height & ~1U;
        video.output_width = video.base_width;
        video.output_height = video.base_height;
        if (strcmp(mode, "display") == 0 || strcmp(mode, "region") == 0)
            quality_limit(quality, &video.output_width, &video.output_height);
        video.output_format = VIDEO_FORMAT_NV12;
        video.gpu_conversion = true;
        video.colorspace = VIDEO_CS_709;
        video.range = VIDEO_RANGE_PARTIAL;
        video.scale_type = OBS_SCALE_BICUBIC;
        int result = obs_reset_video(&video);
        if (result != OBS_VIDEO_SUCCESS) {
            char message[256];
            snprintf(message, sizeof(message), "obs_reset_video for capture target failed (code %d)", result);
            set_error(error, error_size, message);
            return false;
        }
    }
    if (!configure_sources(ctx, mode, target, display_width, display_height,
                           region_x, region_y, region_width, region_height,
                           system_audio, microphone, mic_device, error, error_size))
        return false;
    char first_error[2048] = {0};
    const char *forced_failure = getenv("LUMA_FORCE_ENCODER_FAILURE");
    bool force_this_encoder = forced_failure && strcmp(forced_failure, requested) == 0;
    if (force_this_encoder)
        snprintf(first_error, sizeof(first_error), "forced encoder failure for regression: %s", requested);
    if (!force_this_encoder && start_with_encoder(ctx, path, requested, audio_only, first_error, sizeof(first_error))) {
        if (strcmp(mode, "game") == 0) {
            for (int i = 0; i < 60 && (!ctx->game || !obs_source_get_width(ctx->game) || !obs_source_get_height(ctx->game)); ++i)
                Sleep(100);
            if (!ctx->game || !obs_source_get_width(ctx->game) || !obs_source_get_height(ctx->game)) {
                obs_output_force_stop(ctx->output);
                release_recording(ctx);
                release_dynamic_sources(ctx);
                set_error(error, error_size, "OBS game_capture did not hook the selected target within 6 seconds; run Luma at the same privilege level and ensure the target uses DirectX/OpenGL/Vulkan");
                return false;
            }
        }
        set_error(active, active_size, requested);
        if (fallback && fallback_size) fallback[0] = 0;
        return true;
    }
    if (strcmp(requested, "obs_x264") == 0) {
        set_error(error, error_size, first_error);
        release_dynamic_sources(ctx);
        return false;
    }
    blog(LOG_WARNING, "Luma encoder %s failed, falling back to obs_x264: %s", requested, first_error);
    if (!start_with_encoder(ctx, path, "obs_x264", audio_only, error, error_size)) {
        release_dynamic_sources(ctx);
        return false;
    }
    if (strcmp(mode, "game") == 0) {
        for (int i = 0; i < 60 && (!ctx->game || !obs_source_get_width(ctx->game) || !obs_source_get_height(ctx->game)); ++i)
            Sleep(100);
        if (!ctx->game || !obs_source_get_width(ctx->game) || !obs_source_get_height(ctx->game)) {
            obs_output_force_stop(ctx->output);
            release_recording(ctx);
            release_dynamic_sources(ctx);
            set_error(error, error_size, "OBS game_capture did not hook the selected target within 6 seconds after encoder fallback");
            return false;
        }
    }
    set_error(active, active_size, "obs_x264");
    set_error(fallback, fallback_size, first_error);
    return true;
}

double luma_obs_media_seconds(struct luma_obs *ctx)
{
    if (!ctx || !ctx->output) return 0.0;
    return (double)obs_output_get_total_frames(ctx->output) / 30.0;
}

double luma_obs_wall_seconds(struct luma_obs *ctx)
{
    if (!ctx || !ctx->recording_started_ms) return 0.0;
    ULONGLONG now = GetTickCount64();
    ULONGLONG current_pause = ctx->pause_started_ms ? now - ctx->pause_started_ms : 0;
    return (double)(now - ctx->recording_started_ms - ctx->paused_total_ms - current_pause) / 1000.0;
}

bool luma_obs_pause(struct luma_obs *ctx, bool pause, char *error, size_t error_size)
{
    if (!ctx || !ctx->output || !obs_output_active(ctx->output)) {
        set_error(error, error_size, "no active OBS output to pause");
        return false;
    }
    if ((obs_output_get_flags(ctx->output) & OBS_OUTPUT_CAN_PAUSE) == 0) {
        set_error(error, error_size, "the active OBS output does not support pause");
        return false;
    }
    if (!obs_output_pause(ctx->output, pause)) {
        set_error(error, error_size, pause ? "OBS rejected pause" : "OBS rejected resume");
        return false;
    }
    ULONGLONG now = GetTickCount64();
    if (pause) {
        ctx->pause_started_ms = now;
    } else if (ctx->pause_started_ms) {
        ctx->paused_total_ms += now - ctx->pause_started_ms;
        ctx->pause_started_ms = 0;
    }
    return true;
}

bool luma_obs_stop(struct luma_obs *ctx, uint32_t *encoded_frames, uint64_t *total_bytes,
                   double *media_seconds, double *wall_seconds,
                   char *error, size_t error_size)
{
    if (!ctx || !ctx->output) {
        set_error(error, error_size, "no recording is active");
        return false;
    }
    double stopped_wall_seconds = luma_obs_wall_seconds(ctx);
    obs_output_stop(ctx->output);
    for (int i = 0; i < 150 && obs_output_active(ctx->output); ++i) {
        Sleep(100);
    }
    if (obs_output_active(ctx->output)) {
        obs_output_force_stop(ctx->output);
        set_error(error, error_size, "OBS output did not stop within 15 seconds");
        release_recording(ctx);
        release_dynamic_sources(ctx);
        return false;
    }
    Sleep(500);
    if (encoded_frames) *encoded_frames = ctx->video_encoder ? obs_encoder_get_encoded_frames(ctx->video_encoder) : 0;
    if (total_bytes) *total_bytes = obs_output_get_total_bytes(ctx->output);
    if (media_seconds) *media_seconds = ctx->video_encoder ? (double)obs_output_get_total_frames(ctx->output) / 30.0 : stopped_wall_seconds;
    if (wall_seconds) *wall_seconds = stopped_wall_seconds;
    const char *last_error = obs_output_get_last_error(ctx->output);
    if (last_error && *last_error) {
        set_error(error, error_size, last_error);
        release_recording(ctx);
        release_dynamic_sources(ctx);
        return false;
    }
    release_recording(ctx);
    release_dynamic_sources(ctx);
    ctx->recording_started_ms = 0;
    ctx->pause_started_ms = 0;
    ctx->paused_total_ms = 0;
    return true;
}

void luma_obs_shutdown(struct luma_obs *ctx)
{
    if (!ctx) return;
    if (ctx->output) {
        obs_output_force_stop(ctx->output);
        release_recording(ctx);
    }
    release_dynamic_sources(ctx);
    obs_set_output_source(0, NULL);
    obs_set_output_source(1, NULL);
    if (ctx->scene) obs_scene_release(ctx->scene);
    if (ctx->monitor) obs_source_release(ctx->monitor);
    if (ctx->desktop_audio) obs_source_release(ctx->desktop_audio);
    obs_shutdown();
    free(ctx);
}

#pragma once

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define EXPORT __declspec(dllexport)
#define MODULE_EXPORT EXPORT
#define UNUSED_PARAMETER(param) (void)(param)

#define LOG_ERROR   100
#define LOG_WARNING 200
#define LOG_INFO    300
#define LOG_DEBUG   400

enum obs_source_type {
    OBS_SOURCE_TYPE_INPUT,
    OBS_SOURCE_TYPE_FILTER,
    OBS_SOURCE_TYPE_TRANSITION,
    OBS_SOURCE_TYPE_SCENE,
};

enum obs_base_effect {
    OBS_EFFECT_DEFAULT,
    OBS_EFFECT_DEFAULT_RECT,
    OBS_EFFECT_OPAQUE,
    OBS_EFFECT_SOLID,
    OBS_EFFECT_BICUBIC,
    OBS_EFFECT_LANCZOS,
    OBS_EFFECT_BILINEAR_LOWRES,
    OBS_EFFECT_PREMULTIPLIED_ALPHA,
    OBS_EFFECT_REPEAT,
    OBS_EFFECT_AREA,
};

enum gs_color_format {
    GS_UNKNOWN,
    GS_A8,
    GS_R8,
    GS_RGBA,
    GS_BGRX,
    GS_BGRA,
    GS_R10G10B10A2,
    GS_RGBA16,
    GS_R16,
    GS_RGBA16F,
    GS_RGBA32F,
    GS_RG16F,
    GS_RG32F,
    GS_R16F,
    GS_R32F,
    GS_DXT1,
    GS_DXT3,
    GS_DXT5,
    GS_R8G8,
    GS_RGBA_UNORM,
    GS_BGRX_UNORM,
    GS_BGRA_UNORM,
    GS_RG16,
};

enum obs_allow_direct_render {
    OBS_NO_DIRECT_RENDERING,
    OBS_ALLOW_DIRECT_RENDERING,
};

enum obs_combo_type {
    OBS_COMBO_TYPE_INVALID,
    OBS_COMBO_TYPE_EDITABLE,
    OBS_COMBO_TYPE_LIST,
};

enum obs_combo_format {
    OBS_COMBO_FORMAT_INVALID,
    OBS_COMBO_FORMAT_INT,
    OBS_COMBO_FORMAT_FLOAT,
    OBS_COMBO_FORMAT_STRING,
};

enum obs_icon_type {
    OBS_ICON_TYPE_UNKNOWN,
    OBS_ICON_TYPE_IMAGE,
    OBS_ICON_TYPE_COLOR,
    OBS_ICON_TYPE_SLIDESHOW,
    OBS_ICON_TYPE_AUDIO_INPUT,
    OBS_ICON_TYPE_AUDIO_OUTPUT,
    OBS_ICON_TYPE_DESKTOP_CAPTURE,
    OBS_ICON_TYPE_WINDOW_CAPTURE,
    OBS_ICON_TYPE_GAME_CAPTURE,
    OBS_ICON_TYPE_CAMERA,
    OBS_ICON_TYPE_TEXT,
    OBS_ICON_TYPE_MEDIA,
    OBS_ICON_TYPE_BROWSER,
    OBS_ICON_TYPE_CUSTOM,
    OBS_ICON_TYPE_PROCESS_AUDIO_OUTPUT,
};

enum obs_media_state {
    OBS_MEDIA_STATE_NONE,
    OBS_MEDIA_STATE_PLAYING,
    OBS_MEDIA_STATE_OPENING,
    OBS_MEDIA_STATE_BUFFERING,
    OBS_MEDIA_STATE_PAUSED,
    OBS_MEDIA_STATE_STOPPED,
    OBS_MEDIA_STATE_ENDED,
    OBS_MEDIA_STATE_ERROR,
};

#define OBS_SOURCE_VIDEO (1 << 0)
#define OBS_SOURCE_AUDIO (1 << 1)
#define OBS_SOURCE_ASYNC (1 << 2)

typedef struct obs_source obs_source_t;
typedef struct obs_data obs_data_t;
typedef struct obs_properties obs_properties_t;
typedef struct obs_property obs_property_t;
typedef struct gs_effect gs_effect_t;
typedef struct gs_effect_param gs_eparam_t;
typedef struct gs_texture gs_texture_t;
typedef struct obs_module obs_module_t;
typedef struct obs_missing_files obs_missing_files_t;

struct vec2 {
    union {
        struct {
            float x, y;
        };
        float ptr[2];
    };
};

static inline void vec2_set(struct vec2 *dst, float x, float y)
{
    dst->x = x;
    dst->y = y;
}

struct vec4 {
    union {
        struct {
            float x, y, z, w;
        };
        float ptr[4];
    };
};

static inline void vec4_set(struct vec4 *dst, float x, float y, float z, float w)
{
    dst->x = x;
    dst->y = y;
    dst->z = z;
    dst->w = w;
}

struct obs_source_frame;
struct obs_audio_data;
typedef void (*obs_source_enum_proc_t)(obs_source_t *, obs_source_t *, void *);

struct obs_source_info {
    const char *id;
    enum obs_source_type type;
    uint32_t output_flags;
    const char *(*get_name)(void *type_data);
    void *(*create)(obs_data_t *settings, obs_source_t *source);
    void (*destroy)(void *data);
    uint32_t (*get_width)(void *data);
    uint32_t (*get_height)(void *data);
    void (*get_defaults)(obs_data_t *settings);
    obs_properties_t *(*get_properties)(void *data);
    void (*update)(void *data, obs_data_t *settings);
    void (*activate)(void *data);
    void (*deactivate)(void *data);
    void (*show)(void *data);
    void (*hide)(void *data);
    void (*video_tick)(void *data, float seconds);
    void (*video_render)(void *data, gs_effect_t *effect);
    struct obs_source_frame *(*filter_video)(void *data, struct obs_source_frame *frame);
    struct obs_audio_data *(*filter_audio)(void *data, struct obs_audio_data *audio);
    void (*enum_active_sources)(void *data, obs_source_enum_proc_t enum_callback, void *param);
    void (*save)(void *data, obs_data_t *settings);
    void (*load)(void *data, obs_data_t *settings);
    void (*mouse_click)(void *data, const void *event, int32_t type, bool mouse_up, uint32_t click_count);
    void (*mouse_move)(void *data, const void *event, bool mouse_leave);
    void (*mouse_wheel)(void *data, const void *event, int x_delta, int y_delta);
    void (*focus)(void *data, bool focus);
    void (*key_click)(void *data, const void *event, bool key_up);
    void (*filter_remove)(void *data, obs_source_t *source);
    void *type_data;
    void (*free_type_data)(void *type_data);
    bool (*audio_render)(void *data, uint64_t *ts_out, void *audio_output, uint32_t mixers, size_t channels, size_t sample_rate);
    void (*enum_all_sources)(void *data, obs_source_enum_proc_t enum_callback, void *param);
    void (*transition_start)(void *data);
    void (*transition_stop)(void *data);
    void (*get_defaults2)(void *type_data, obs_data_t *settings);
    obs_properties_t *(*get_properties2)(void *data, void *type_data);
    bool (*audio_mix)(void *data, uint64_t *ts_out, void *audio_output, size_t channels, size_t sample_rate);
    enum obs_icon_type icon_type;
    void (*media_play_pause)(void *data, bool pause);
    void (*media_restart)(void *data);
    void (*media_stop)(void *data);
    void (*media_next)(void *data);
    void (*media_previous)(void *data);
    int64_t (*media_get_duration)(void *data);
    int64_t (*media_get_time)(void *data);
    void (*media_set_time)(void *data, int64_t miliseconds);
    enum obs_media_state (*media_get_state)(void *data);
    uint32_t version;
    const char *unversioned_id;
    obs_missing_files_t *(*missing_files)(void *data);
    int (*video_get_color_space)(void *data, size_t count, const void *preferred_spaces);
    void (*filter_add)(void *data, obs_source_t *source);
    const char *(*get_dark_icon)(void *type_data);
    const char *(*get_light_icon)(void *type_data);
};

// Functions imported from obs.dll
__declspec(dllimport) void obs_register_source_s(const struct obs_source_info *info, size_t size);
#define obs_register_source(info) obs_register_source_s(info, sizeof(struct obs_source_info))

__declspec(dllimport) void blog(int log_level, const char *format, ...);
#include <stdlib.h>
#define bzalloc(size) calloc(1, size)
#define bfree(ptr) free(ptr)

__declspec(dllimport) const char *obs_source_get_name(const obs_source_t *source);
__declspec(dllimport) obs_source_t *obs_filter_get_target(const obs_source_t *filter);
__declspec(dllimport) uint32_t obs_source_get_base_width(const obs_source_t *source);
__declspec(dllimport) uint32_t obs_source_get_base_height(const obs_source_t *source);
__declspec(dllimport) bool obs_source_process_filter_begin(obs_source_t *filter, enum gs_color_format format, enum obs_allow_direct_render allow_direct);
__declspec(dllimport) void obs_source_process_filter_end(obs_source_t *filter, gs_effect_t *effect, uint32_t width, uint32_t height);
__declspec(dllimport) void obs_source_skip_video_filter(obs_source_t *filter);
__declspec(dllimport) void obs_source_update(obs_source_t *source, obs_data_t *settings);

__declspec(dllimport) void obs_enter_graphics(void);
__declspec(dllimport) void obs_leave_graphics(void);
__declspec(dllimport) gs_effect_t *gs_effect_create(const char *effect_string, const char *filename, char **error_string);
__declspec(dllimport) void gs_effect_destroy(gs_effect_t *effect);
__declspec(dllimport) gs_eparam_t *gs_effect_get_param_by_name(const gs_effect_t *effect, const char *name);
__declspec(dllimport) void gs_effect_set_vec4(gs_eparam_t *param, const struct vec4 *val);
__declspec(dllimport) void gs_effect_set_vec2(gs_eparam_t *param, const struct vec2 *val);
__declspec(dllimport) void gs_effect_set_float(gs_eparam_t *param, float val);
__declspec(dllimport) void gs_effect_set_int(gs_eparam_t *param, int val);
__declspec(dllimport) void gs_effect_set_bool(gs_eparam_t *param, bool val);
__declspec(dllimport) void gs_effect_set_val(gs_eparam_t *param, const void *val, size_t size);
__declspec(dllimport) bool gs_effect_loop(gs_effect_t *effect, const char *name);

__declspec(dllimport) void gs_matrix_push(void);
__declspec(dllimport) void gs_matrix_pop(void);
__declspec(dllimport) void gs_matrix_identity(void);
__declspec(dllimport) void gs_matrix_translate3f(float x, float y, float z);
__declspec(dllimport) void gs_draw_sprite(gs_texture_t *tex, uint32_t flip, uint32_t width, uint32_t height);
__declspec(dllimport) void gs_ortho(float left, float right, float top, float bottom, float znear, float zfar);

typedef struct gs_texture_render gs_texrender_t;
typedef struct gs_stage_surface gs_stagesurf_t;

__declspec(dllimport) gs_texrender_t *gs_texrender_create(enum gs_color_format format, int zsformat);
__declspec(dllimport) void gs_texrender_destroy(gs_texrender_t *texrender);
__declspec(dllimport) bool gs_texrender_begin(gs_texrender_t *texrender, uint32_t cx, uint32_t cy);
__declspec(dllimport) void gs_texrender_end(gs_texrender_t *texrender);
__declspec(dllimport) gs_texture_t *gs_texrender_get_texture(const gs_texrender_t *texrender);

__declspec(dllimport) gs_stagesurf_t *gs_stagesurface_create(uint32_t width, uint32_t height, enum gs_color_format color_format);
__declspec(dllimport) void gs_stagesurface_destroy(gs_stagesurf_t *stagesurf);
__declspec(dllimport) void gs_stage_texture(gs_stagesurf_t *dst, gs_texture_t *src);
__declspec(dllimport) bool gs_stagesurface_map(gs_stagesurf_t *stagesurf, uint8_t **data, uint32_t *linesize);
__declspec(dllimport) void gs_stagesurface_unmap(gs_stagesurf_t *stagesurf);
__declspec(dllimport) void *gs_effect_get_val(gs_eparam_t *param);
#define GS_CLEAR_COLOR (1 << 0)
__declspec(dllimport) void obs_source_video_render(obs_source_t *source);
__declspec(dllimport) void gs_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil);
__declspec(dllimport) void obs_source_process_filter_tech_end(obs_source_t *filter, gs_effect_t *effect, uint32_t width, uint32_t height, const char *tech_name);

__declspec(dllimport) obs_properties_t *obs_properties_create(void);
__declspec(dllimport) obs_property_t *obs_properties_add_int(obs_properties_t *props, const char *name, const char *description, int min, int max, int step);
__declspec(dllimport) obs_property_t *obs_properties_add_float_slider(obs_properties_t *props, const char *name, const char *description, double min, double max, double step);
__declspec(dllimport) obs_property_t *obs_properties_add_int_slider(obs_properties_t *props, const char *name, const char *description, int min, int max, int step);
__declspec(dllimport) obs_property_t *obs_properties_add_bool(obs_properties_t *props, const char *name, const char *description);
__declspec(dllimport) obs_property_t *obs_properties_add_list(obs_properties_t *props, const char *name, const char *description, enum obs_combo_type type, enum obs_combo_format format);
__declspec(dllimport) size_t obs_property_list_add_int(obs_property_t *p, const char *name, long long val);

__declspec(dllimport) long long obs_data_get_int(obs_data_t *data, const char *name);
__declspec(dllimport) double obs_data_get_double(obs_data_t *data, const char *name);
__declspec(dllimport) bool obs_data_get_bool(obs_data_t *data, const char *name);
__declspec(dllimport) void obs_data_set_default_int(obs_data_t *data, const char *name, long long val);
__declspec(dllimport) void obs_data_set_default_double(obs_data_t *data, const char *name, double val);
__declspec(dllimport) void obs_data_set_default_bool(obs_data_t *data, const char *name, bool val);

static obs_module_t *obs_current_module_ptr = NULL;

#ifdef __cplusplus
}
#endif

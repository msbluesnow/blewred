#include "obs-plugin-api.h"
#include <stdio.h>
#include <string.h>
#include <stdbool.h>
#include <winsock2.h>
#include <ws2tcpip.h>

#define BLEWRED_DEFAULT_PORT 51799
#define BLEWRED_HEARTBEAT_PORT 51798
#define BLEWRED_MAX_BOXES 16
#define BLEWRED_BOX_HOLD_MS 500
#define BLEWRED_FADE_FRAMES 12

static const char *BLEWRED_EFFECT_SRC = 
"uniform float4x4 ViewProj;\n"
"uniform texture2d image;\n"
"\n"
"uniform float4 box_0;\n"
"uniform float4 box_1;\n"
"uniform float4 box_2;\n"
"uniform float4 box_3;\n"
"uniform float4 box_4;\n"
"uniform float4 box_5;\n"
"uniform float4 box_6;\n"
"uniform float4 box_7;\n"
"uniform float4 box_8;\n"
"uniform float4 box_9;\n"
"uniform float4 box_10;\n"
"uniform float4 box_11;\n"
"uniform float4 box_12;\n"
"uniform float4 box_13;\n"
"uniform float4 box_14;\n"
"uniform float4 box_15;\n"
"\n"
"uniform int box_count;\n"
"uniform int censor_mode;\n"
"uniform float blur_radius;\n"
"uniform float pixel_size;\n"
"uniform float box_padding;\n"
"uniform int invert_y;\n"
"uniform int show_debug;\n"
"uniform int censor_all;\n"
"uniform float2 image_size;\n"
"\n"
"sampler_state def_sampler {\n"
"    Filter   = Linear;\n"
"    AddressU = Clamp;\n"
"    AddressV = Clamp;\n"
"};\n"
"\n"
"struct VertInOut {\n"
"    float4 pos : POSITION;\n"
"    float2 uv  : TEXCOORD0;\n"
"};\n"
"\n"
"VertInOut VSDefault(VertInOut vert_in)\n"
"{\n"
"    VertInOut vert_out;\n"
"    vert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n"
"    vert_out.uv  = vert_in.uv;\n"
"    return vert_out;\n"
"}\n"
"\n"
"float4 get_box(int i)\n"
"{\n"
"    if (i == 0) return box_0;\n"
"    if (i == 1) return box_1;\n"
"    if (i == 2) return box_2;\n"
"    if (i == 3) return box_3;\n"
"    if (i == 4) return box_4;\n"
"    if (i == 5) return box_5;\n"
"    if (i == 6) return box_6;\n"
"    if (i == 7) return box_7;\n"
"    if (i == 8) return box_8;\n"
"    if (i == 9) return box_9;\n"
"    if (i == 10) return box_10;\n"
"    if (i == 11) return box_11;\n"
"    if (i == 12) return box_12;\n"
"    if (i == 13) return box_13;\n"
"    if (i == 14) return box_14;\n"
"    return box_15;\n"
"}\n"
"\n"
"float4 PSCensor(VertInOut vert_in) : TARGET\n"
"{\n"
"    bool is_censored = false;\n"
"    bool is_border = false;\n"
"\n"
"    if (censor_all > 0) {\n"
"        is_censored = true;\n"
"    } else {\n"
"        for (int i = 0; i < box_count && i < 16; i++) {\n"
"            float4 b = get_box(i);\n"
"            float x1 = b.x;\n"
"            float y1 = b.y;\n"
"            float x2 = b.z;\n"
"            float y2 = b.w;\n"
"\n"
"            if (x2 <= x1 || y2 <= y1) continue;\n"
"\n"
"            if (invert_y > 0) {\n"
"                float ty1 = 1.0 - y2;\n"
"                float ty2 = 1.0 - y1;\n"
"                y1 = ty1;\n"
"                y2 = ty2;\n"
"            }\n"
"\n"
"            float bw = x2 - x1;\n"
"            float bh = y2 - y1;\n"
"            float px = bw * box_padding;\n"
"            float py = bh * box_padding;\n"
"            x1 = max(0.0, x1 - px);\n"
"            y1 = max(0.0, y1 - py);\n"
"            x2 = min(1.0, x2 + px);\n"
"            y2 = min(1.0, y2 + py);\n"
"\n"
"            if (vert_in.uv.x >= x1 && vert_in.uv.x <= x2 &&\n"
"                vert_in.uv.y >= y1 && vert_in.uv.y <= y2) {\n"
"                is_censored = true;\n"
"\n"
"                if (show_debug > 0) {\n"
"                    float2 safe_sz = max(image_size, float2(320.0, 180.0));\n"
"                    float2 dist_min = (vert_in.uv - float2(x1, y1)) * safe_sz;\n"
"                    float2 dist_max = (float2(x2, y2) - vert_in.uv) * safe_sz;\n"
"                    float d = min(min(dist_min.x, dist_min.y), min(dist_max.x, dist_max.y));\n"
"                    if (d <= 2.5) {\n"
"                        is_border = true;\n"
"                    }\n"
"                }\n"
"                break;\n"
"            }\n"
"        }\n"
"    }\n"
"\n"
"    if (!is_censored) {\n"
"        return image.Sample(def_sampler, vert_in.uv);\n"
"    }\n"
"\n"
"    if (is_border) {\n"
"        return float4(0.0, 1.0, 0.4, 1.0);\n"
"    }\n"
"\n"
"    float2 safe_sz = max(image_size, float2(320.0, 180.0));\n"
"\n"
"    if (censor_mode == 0) {\n"
"        // Smart Gaussian Defocus Blur (13-sample circular kernel)\n"
"        float4 col = float4(0.0, 0.0, 0.0, 0.0);\n"
"        float2 step = max(float2(2.0, 2.0), float2(blur_radius, blur_radius)) / safe_sz;\n"
"\n"
"        col += image.Sample(def_sampler, vert_in.uv) * 0.16;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2( step.x, 0.0)) * 0.12;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2(-step.x, 0.0)) * 0.12;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2(0.0,  step.y)) * 0.12;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2(0.0, -step.y)) * 0.12;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2( step.x * 0.707,  step.y * 0.707)) * 0.09;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2(-step.x * 0.707,  step.y * 0.707)) * 0.09;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2( step.x * 0.707, -step.y * 0.707)) * 0.09;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2(-step.x * 0.707, -step.y * 0.707)) * 0.09;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2( step.x * 1.6, 0.0)) * 0.04;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2(-step.x * 1.6, 0.0)) * 0.04;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2(0.0,  step.y * 1.6)) * 0.04;\n"
"        col += image.Sample(def_sampler, vert_in.uv + float2(0.0, -step.y * 1.6)) * 0.04;\n"
"        return col;\n"
"    } else if (censor_mode == 1) {\n"
"        // Pixelate / Mosaic\n"
"        float2 psize = max(float2(4.0, 4.0), float2(pixel_size, pixel_size)) / safe_sz;\n"
"        float2 block_uv = (floor(vert_in.uv / psize) + 0.5) * psize;\n"
"        return image.Sample(def_sampler, block_uv);\n"
"    } else if (censor_mode == 2) {\n"
"        // Solid Obsidian Black\n"
"        return float4(0.02, 0.02, 0.03, 1.0);\n"
"    } else if (censor_mode == 3) {\n"
"        // Frosted Dark Glass\n"
"        float4 orig = image.Sample(def_sampler, vert_in.uv);\n"
"        return lerp(orig, float4(0.08, 0.11, 0.18, 1.0), 0.88);\n"
"    } else {\n"
"        // Crimson Alert (Streamer Warning)\n"
"        return float4(0.65, 0.05, 0.10, 0.95);\n"
"    }\n"
"}\n"
"\n"
"technique Draw\n"
"{\n"
"    pass\n"
"    {\n"
"        vertex_shader = VSDefault(vert_in);\n"
"        pixel_shader  = PSCensor(vert_in);\n"
"    }\n"
"}\n";

#define BLEWRED_SHM_NAME L"Local\\BlewRed_OBS_Frame_SHM"
#define BLEWRED_SHM_WIDTH 640
#define BLEWRED_SHM_HEIGHT 360
#define BLEWRED_SHM_CHANNELS 4
#define BLEWRED_SHM_DATA_SIZE (BLEWRED_SHM_WIDTH * BLEWRED_SHM_HEIGHT * BLEWRED_SHM_CHANNELS)
#define BLEWRED_SHM_TOTAL_SIZE (sizeof(blewred_shm_header_t) + BLEWRED_SHM_DATA_SIZE)

#pragma pack(push, 1)
typedef struct {
    uint32_t magic;         // 0x424C5752 "BLWR"
    uint32_t version;       // 1
    uint32_t width;         // 640
    uint32_t height;        // 360
    uint32_t stride;        // 640 * 4
    uint32_t format;        // 0 = BGRA, 1 = RGBA
    uint64_t frame_index;   // Incremented per frame
    uint64_t timestamp_ms;  // GetTickCount64()
    uint32_t reserved[8];
} blewred_shm_header_t;
#pragma pack(pop)

/* --------------------------------------------------------------------------
 * Locale-independent float parsing (prevents Russian locale '.' vs ',' bugs)
 * -------------------------------------------------------------------------- */
static float parse_float_fast(const char *str)
{
    if (!str) return 0.0f;
    while (*str == ' ' || *str == '\t') str++;
    float sign = 1.0f;
    if (*str == '-') { sign = -1.0f; str++; }
    else if (*str == '+') { str++; }
    
    double val = 0.0;
    while (*str >= '0' && *str <= '9') {
        val = val * 10.0 + (*str - '0');
        str++;
    }
    if (*str == '.' || *str == ',') {
        str++;
        double factor = 0.1;
        while (*str >= '0' && *str <= '9') {
            val += (*str - '0') * factor;
            factor *= 0.1;
            str++;
        }
    }
    return (float)(sign * val);
}

static float parse_json_obj_field(const char *obj_start, const char *obj_end, const char *key, bool *found)
{
    if (found) *found = false;
    size_t klen = strlen(key);
    const char *p = obj_start;
    while (p < obj_end) {
        const char *match = strstr(p, key);
        if (!match || match >= obj_end) break;
        
        const char *val = match + klen;
        while (val < obj_end && (*val == ' ' || *val == ':' || *val == '\t')) {
            val++;
        }
        if (val < obj_end && ((*val >= '0' && *val <= '9') || *val == '-' || *val == '+')) {
            if (found) *found = true;
            return parse_float_fast(val);
        }
        p = match + klen;
    }
    return 0.0f;
}

/* --------------------------------------------------------------------------
 * Global Singleton UDP Receiver & Shared Box State
 * Resolves multi-source socket port 51799 binding collisions across filters.
 * -------------------------------------------------------------------------- */
typedef struct {
    CRITICAL_SECTION cs;
    SOCKET sock;
    int port;
    LONG ref_count;
    
    // Shared incoming state
    bool censor_all;
    uint32_t target_box_count;
    struct vec4 target_boxes[BLEWRED_MAX_BOXES];
    uint64_t last_packet_time;
    uint64_t last_heartbeat_time;
    uint64_t last_box_seen_time;
    uint64_t last_parse_log;
} blewred_shared_udp_t;

static blewred_shared_udp_t g_shared_udp = {0};

static void parse_json_boxes_to_shared(const char *buf, uint64_t now)
{
    EnterCriticalSection(&g_shared_udp.cs);
    
    g_shared_udp.censor_all = false;
    g_shared_udp.target_box_count = 0;
    
    if (strstr(buf, "\"censor_all\":true") || strstr(buf, "\"censor_all\": true")) {
        g_shared_udp.censor_all = true;
    }
    
    const char *boxes_start = strstr(buf, "\"boxes\":");
    if (boxes_start) {
        const char *arr_open = strchr(boxes_start, '[');
        if (arr_open) {
            const char *arr_end = NULL;
            int depth = 0;
            for (const char *scan = arr_open; *scan; scan++) {
                if (*scan == '[') depth++;
                else if (*scan == ']') {
                    depth--;
                    if (depth == 0) { arr_end = scan; break; }
                }
            }
            if (!arr_end) arr_end = buf + strlen(buf);

            const char *p = arr_open + 1;
            while (*p && p < arr_end && g_shared_udp.target_box_count < BLEWRED_MAX_BOXES) {
                const char *obj_start = strchr(p, '{');
                if (!obj_start || obj_start >= arr_end) break;
                
                const char *obj_end = strchr(obj_start, '}');
                if (!obj_end || obj_end >= arr_end) break;
                
                bool f_x1 = false, f_y1 = false, f_x2 = false, f_y2 = false;
                float x1 = parse_json_obj_field(obj_start, obj_end, "\"x1\"", &f_x1);
                float y1 = parse_json_obj_field(obj_start, obj_end, "\"y1\"", &f_y1);
                float x2 = parse_json_obj_field(obj_start, obj_end, "\"x2\"", &f_x2);
                float y2 = parse_json_obj_field(obj_start, obj_end, "\"y2\"", &f_y2);
                
                if (f_x1 && f_y1 && f_x2 && f_y2) {
                    if (x1 < 0.0f) x1 = 0.0f;
                    if (y1 < 0.0f) y1 = 0.0f;
                    if (x2 > 1.0f) x2 = 1.0f;
                    if (y2 > 1.0f) y2 = 1.0f;
                    
                    if (x2 > x1 && y2 > y1) {
                        vec4_set(&g_shared_udp.target_boxes[g_shared_udp.target_box_count], x1, y1, x2, y2);
                        g_shared_udp.target_box_count++;
                    }
                }
                
                p = obj_end + 1;
            }
            
            if (g_shared_udp.target_box_count > 0) {
                g_shared_udp.last_box_seen_time = now;
            }
        }
    }
    
    g_shared_udp.last_packet_time = now;
    
    if (g_shared_udp.target_box_count > 0 || g_shared_udp.censor_all) {
        if (now - g_shared_udp.last_parse_log > 2000) {
            blog(LOG_INFO, "[BlewRed Filter] Active packet parsed: %u boxes (censor_all=%d, box0=[%.2f,%.2f,%.2f,%.2f])",
                 g_shared_udp.target_box_count, g_shared_udp.censor_all ? 1 : 0,
                 g_shared_udp.target_boxes[0].x, g_shared_udp.target_boxes[0].y,
                 g_shared_udp.target_boxes[0].z, g_shared_udp.target_boxes[0].w);
            g_shared_udp.last_parse_log = now;
        }
    }
    
    LeaveCriticalSection(&g_shared_udp.cs);
}

static void shared_udp_init(void)
{
    if (InterlockedIncrement(&g_shared_udp.ref_count) == 1) {
        InitializeCriticalSection(&g_shared_udp.cs);
        g_shared_udp.port = BLEWRED_DEFAULT_PORT;
        g_shared_udp.sock = INVALID_SOCKET;
        
        WSADATA wsa;
        WSAStartup(MAKEWORD(2, 2), &wsa);
        
        g_shared_udp.sock = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
        if (g_shared_udp.sock != INVALID_SOCKET) {
            u_long mode = 1;
            ioctlsocket(g_shared_udp.sock, FIONBIO, &mode);
            
            BOOL reuse = TRUE;
            setsockopt(g_shared_udp.sock, SOL_SOCKET, SO_REUSEADDR, (const char*)&reuse, sizeof(reuse));
            
            struct sockaddr_in addr;
            memset(&addr, 0, sizeof(addr));
            addr.sin_family = AF_INET;
            addr.sin_port = htons(g_shared_udp.port);
            addr.sin_addr.s_addr = htonl(INADDR_ANY);
            
            if (bind(g_shared_udp.sock, (struct sockaddr*)&addr, sizeof(addr)) == SOCKET_ERROR) {
                blog(LOG_WARNING, "[BlewRed Filter] Could not bind shared UDP port %d: %d", g_shared_udp.port, WSAGetLastError());
            } else {
                blog(LOG_INFO, "[BlewRed Filter] Bound shared UDP receiver on 0.0.0.0:%d", g_shared_udp.port);
            }
        }
    }
}

static void shared_udp_cleanup(void)
{
    if (InterlockedDecrement(&g_shared_udp.ref_count) == 0) {
        if (g_shared_udp.sock != INVALID_SOCKET) {
            closesocket(g_shared_udp.sock);
            g_shared_udp.sock = INVALID_SOCKET;
        }
        DeleteCriticalSection(&g_shared_udp.cs);
    }
}

static void shared_udp_poll(uint64_t now)
{
    if (g_shared_udp.sock == INVALID_SOCKET) return;
    
    // Heartbeat every 500ms
    if (now - g_shared_udp.last_heartbeat_time >= 500) {
        struct sockaddr_in dest;
        memset(&dest, 0, sizeof(dest));
        dest.sin_family = AF_INET;
        dest.sin_port = htons(BLEWRED_HEARTBEAT_PORT);
        dest.sin_addr.s_addr = inet_addr("127.0.0.1");
        
        const char *hb = "{\"type\":\"obs_filter_heartbeat\",\"status\":\"obs_attached\",\"version\":\"1.2.0\"}";
        sendto(g_shared_udp.sock, hb, (int)strlen(hb), 0, (struct sockaddr*)&dest, sizeof(dest));
        g_shared_udp.last_heartbeat_time = now;
    }
    
    // Poll non-blocking UDP packets
    char buf[8192];
    struct sockaddr_in sender;
    int sender_len = sizeof(sender);
    
    while (1) {
        int len = recvfrom(g_shared_udp.sock, buf, sizeof(buf) - 1, 0, (struct sockaddr*)&sender, &sender_len);
        if (len > 0) {
            buf[len] = '\0';
            parse_json_boxes_to_shared(buf, now);
        } else {
            break;
        }
    }
}

/* --------------------------------------------------------------------------
 * Per-Filter Instance Data
 * -------------------------------------------------------------------------- */
typedef struct {
    obs_source_t *context;
    gs_effect_t *effect;
    
    gs_eparam_t *param_boxes[BLEWRED_MAX_BOXES];
    gs_eparam_t *param_box_count;
    gs_eparam_t *param_censor_mode;
    gs_eparam_t *param_blur_radius;
    gs_eparam_t *param_pixel_size;
    gs_eparam_t *param_box_padding;
    gs_eparam_t *param_invert_y;
    gs_eparam_t *param_show_debug;
    gs_eparam_t *param_censor_all;
    gs_eparam_t *param_image_size;
    
    int censor_mode;
    float blur_radius;
    float pixel_size;
    float box_padding;
    float smoothing;
    bool invert_y;
    bool show_debug;
    int port;
    
    CRITICAL_SECTION cs;
    
    struct vec4 current_boxes[BLEWRED_MAX_BOXES];
    struct vec4 target_boxes[BLEWRED_MAX_BOXES];
    uint32_t current_box_count;
    uint32_t target_box_count;
    bool censor_all;
    uint64_t last_packet_time;
    uint64_t last_box_seen_time;
    uint32_t fade_counter;
    uint32_t held_box_count;
    
    // Native source video capture for AI Core
    gs_texrender_t *texrender;
    gs_stagesurf_t *stagesurf;
    HANDLE shm_handle;
    void *shm_ptr;
    uint64_t last_capture_time;
    uint64_t frame_index;
} blewred_filter_data_t;

static const char *blewred_get_name(void *unused)
{
    UNUSED_PARAMETER(unused);
    return "BlewRed AI Smart Shield";
}

static void blewred_video_tick(void *data, float seconds)
{
    UNUSED_PARAMETER(seconds);
    blewred_filter_data_t *filter = (blewred_filter_data_t *)data;
    if (!filter) return;
    
    uint64_t now = GetTickCount64();
    
    // Poll singleton UDP receiver
    shared_udp_poll(now);
    
    // Sync latest target boxes from shared UDP state
    EnterCriticalSection(&g_shared_udp.cs);
    filter->censor_all = g_shared_udp.censor_all;
    filter->target_box_count = g_shared_udp.target_box_count;
    for (uint32_t i = 0; i < g_shared_udp.target_box_count; i++) {
        filter->target_boxes[i] = g_shared_udp.target_boxes[i];
    }
    filter->last_packet_time = g_shared_udp.last_packet_time;
    if (g_shared_udp.target_box_count > 0) {
        filter->last_box_seen_time = g_shared_udp.last_box_seen_time;
    }
    LeaveCriticalSection(&g_shared_udp.cs);
    
    // Exponential moving average smoothing & persistence decay hold for bounding boxes
    EnterCriticalSection(&filter->cs);
    float alpha = 1.0f - (filter->smoothing / 100.0f);
    alpha = (alpha < 0.05f) ? 0.05f : ((alpha > 1.0f) ? 1.0f : alpha);
    
    if (filter->target_box_count > 0) {
        // Active detections: smooth-interpolate boxes toward targets
        filter->current_box_count = filter->target_box_count;
        filter->held_box_count = filter->target_box_count;
        filter->fade_counter = 0;
        for (uint32_t i = 0; i < filter->target_box_count; i++) {
            filter->current_boxes[i].x += (filter->target_boxes[i].x - filter->current_boxes[i].x) * alpha;
            filter->current_boxes[i].y += (filter->target_boxes[i].y - filter->current_boxes[i].y) * alpha;
            filter->current_boxes[i].z += (filter->target_boxes[i].z - filter->current_boxes[i].z) * alpha;
            filter->current_boxes[i].w += (filter->target_boxes[i].w - filter->current_boxes[i].w) * alpha;
        }
    } else if (now - filter->last_box_seen_time < BLEWRED_BOX_HOLD_MS) {
        // Grace period: hold last known boxes as-is to bridge inference gaps
        filter->current_box_count = filter->held_box_count;
    } else if (filter->fade_counter < BLEWRED_FADE_FRAMES && filter->held_box_count > 0) {
        // Smooth fade-out: gradually shrink boxes to center over FADE_FRAMES ticks
        filter->fade_counter++;
        float fade_t = (float)filter->fade_counter / (float)BLEWRED_FADE_FRAMES;
        filter->current_box_count = filter->held_box_count;
        for (uint32_t i = 0; i < filter->held_box_count; i++) {
            float cx = (filter->current_boxes[i].x + filter->current_boxes[i].z) * 0.5f;
            float cy = (filter->current_boxes[i].y + filter->current_boxes[i].w) * 0.5f;
            filter->current_boxes[i].x += (cx - filter->current_boxes[i].x) * fade_t * 0.3f;
            filter->current_boxes[i].y += (cy - filter->current_boxes[i].y) * fade_t * 0.3f;
            filter->current_boxes[i].z += (cx - filter->current_boxes[i].z) * fade_t * 0.3f;
            filter->current_boxes[i].w += (cy - filter->current_boxes[i].w) * fade_t * 0.3f;
        }
    } else {
        // Fade complete: cleanly remove all censor boxes
        filter->current_box_count = 0;
        filter->held_box_count = 0;
    }
    LeaveCriticalSection(&filter->cs);
}

static void blewred_video_render(void *data, gs_effect_t *effect)
{
    UNUSED_PARAMETER(effect);
    blewred_filter_data_t *filter = (blewred_filter_data_t *)data;
    if (!filter || !filter->context || !filter->effect) {
        obs_source_skip_video_filter(filter ? filter->context : NULL);
        return;
    }
    
    uint64_t now = GetTickCount64();
    obs_source_t *target = obs_filter_get_target(filter->context);
    uint32_t width = target ? obs_source_get_base_width(target) : 0;
    uint32_t height = target ? obs_source_get_base_height(target) : 0;
    if (!width && target) width = obs_source_get_width(target);
    if (!height && target) height = obs_source_get_height(target);
    if (!width) width = 1920;
    if (!height) height = 1080;

    // 1. CAPTURE & STREAM SOURCE VIDEO FRAME TO SHARED MEMORY (up to 60 FPS)
    if (filter->texrender && filter->stagesurf && filter->shm_ptr && (now - filter->last_capture_time >= 15)) {
        static uint64_t last_global_capture_time = 0;
        if (now - last_global_capture_time >= 15) {
            last_global_capture_time = now;
            filter->last_capture_time = now;
            
            if (target && gs_texrender_begin(filter->texrender, BLEWRED_SHM_WIDTH, BLEWRED_SHM_HEIGHT)) {
                struct vec4 black;
                vec4_set(&black, 0.0f, 0.0f, 0.0f, 0.0f);
                gs_clear(GS_CLEAR_COLOR, &black, 0.0f, 0);
                gs_ortho(0.0f, (float)width, 0.0f, (float)height, -100.0f, 100.0f);
                
                obs_source_video_render(target);
                gs_texrender_end(filter->texrender);
                
                gs_texture_t *tex = gs_texrender_get_texture(filter->texrender);
                if (tex) {
                    gs_stage_texture(filter->stagesurf, tex);
                    uint8_t *mapped = NULL;
                    uint32_t linesize = 0;
                    if (gs_stagesurface_map(filter->stagesurf, &mapped, &linesize)) {
                        blewred_shm_header_t *hdr = (blewred_shm_header_t *)filter->shm_ptr;
                        uint8_t *dst = (uint8_t *)filter->shm_ptr + sizeof(blewred_shm_header_t);
                        for (uint32_t y = 0; y < BLEWRED_SHM_HEIGHT; y++) {
                            memcpy(dst + y * (BLEWRED_SHM_WIDTH * 4), mapped + y * linesize, BLEWRED_SHM_WIDTH * 4);
                        }
                        filter->frame_index++;
                        hdr->frame_index = filter->frame_index;
                        hdr->timestamp_ms = now;
                        gs_stagesurface_unmap(filter->stagesurf);
                    }
                }
            }
        }
    }

    // 2. RENDER SHIELD CENSOR SHADER
    bool active = (now - filter->last_packet_time) < 1500;
    
    // Zero FPS overhead if clean
    if (!active || (filter->current_box_count == 0 && !filter->censor_all)) {
        obs_source_skip_video_filter(filter->context);
        return;
    }
    
    if (!obs_source_process_filter_begin(filter->context, GS_RGBA, OBS_NO_DIRECT_RENDERING))
        return;
        
    static uint64_t last_dbg_log = 0;
    if (now - last_dbg_log > 2000) {
        blog(LOG_INFO, "[BlewRed Filter] SUCCESS Rendering: boxes=%u, censor_all=%d, dim=%ux%u",
             filter->current_box_count, filter->censor_all ? 1 : 0, width, height);
        last_dbg_log = now;
    }

    EnterCriticalSection(&filter->cs);
    for (uint32_t i = 0; i < BLEWRED_MAX_BOXES; i++) {
        if (filter->param_boxes[i]) {
            if (i < filter->current_box_count) {
                gs_effect_set_vec4(filter->param_boxes[i], &filter->current_boxes[i]);
            } else {
                struct vec4 zero;
                vec4_set(&zero, 0.0f, 0.0f, 0.0f, 0.0f);
                gs_effect_set_vec4(filter->param_boxes[i], &zero);
            }
        }
    }
    if (filter->param_box_count)
        gs_effect_set_int(filter->param_box_count, (int)filter->current_box_count);
    if (filter->param_censor_mode)
        gs_effect_set_int(filter->param_censor_mode, filter->censor_mode);
    if (filter->param_blur_radius)
        gs_effect_set_float(filter->param_blur_radius, filter->blur_radius);
    if (filter->param_pixel_size)
        gs_effect_set_float(filter->param_pixel_size, filter->pixel_size);
    if (filter->param_box_padding)
        gs_effect_set_float(filter->param_box_padding, filter->box_padding / 100.0f);
    if (filter->param_invert_y)
        gs_effect_set_int(filter->param_invert_y, filter->invert_y ? 1 : 0);
    if (filter->param_show_debug)
        gs_effect_set_int(filter->param_show_debug, filter->show_debug ? 1 : 0);
    if (filter->param_censor_all)
        gs_effect_set_int(filter->param_censor_all, filter->censor_all ? 1 : 0);
    if (filter->param_image_size) {
        struct vec2 sz;
        vec2_set(&sz, (float)width, (float)height);
        gs_effect_set_vec2(filter->param_image_size, &sz);
    }
    LeaveCriticalSection(&filter->cs);
    
    obs_source_process_filter_tech_end(filter->context, filter->effect, width, height, "Draw");
}

static void blewred_filter_update(void *data, obs_data_t *settings)
{
    blewred_filter_data_t *filter = (blewred_filter_data_t *)data;
    if (!filter || !settings) return;
    
    filter->port = (int)obs_data_get_int(settings, "udp_port");
    if (filter->port <= 1024 || filter->port > 65535) filter->port = BLEWRED_DEFAULT_PORT;
    
    filter->censor_mode = (int)obs_data_get_int(settings, "censor_mode");
    filter->blur_radius = (float)obs_data_get_double(settings, "blur_radius");
    filter->pixel_size = (float)obs_data_get_double(settings, "pixel_size");
    filter->box_padding = (float)obs_data_get_double(settings, "box_padding");
    filter->smoothing = (float)obs_data_get_double(settings, "smoothing");
    filter->invert_y = obs_data_get_bool(settings, "invert_y");
    filter->show_debug = obs_data_get_bool(settings, "show_debug");
}

static void *blewred_filter_create(obs_data_t *settings, obs_source_t *context)
{
    blewred_filter_data_t *filter = bzalloc(sizeof(*filter));
    filter->context = context;
    filter->port = BLEWRED_DEFAULT_PORT;
    filter->censor_mode = 0; // 0 = Smart Blur by default
    filter->blur_radius = 24.0f;
    filter->pixel_size = 20.0f;
    filter->box_padding = 15.0f;
    filter->smoothing = 40.0f;
    filter->invert_y = false;
    filter->show_debug = false;
    InitializeCriticalSection(&filter->cs);
    
    // Compile HLSL GPU Effect
    obs_enter_graphics();
    char *err = NULL;
    filter->effect = gs_effect_create(BLEWRED_EFFECT_SRC, "blewred_censor", &err);
    if (!filter->effect) {
        blog(LOG_ERROR, "[BlewRed Filter] Shader compilation error: %s", err ? err : "unknown");
    } else {
        blog(LOG_INFO, "[BlewRed Filter] GPU Censor shader compiled successfully!");
        for (int i = 0; i < BLEWRED_MAX_BOXES; i++) {
            char name[32];
            snprintf(name, sizeof(name), "box_%d", i);
            filter->param_boxes[i] = gs_effect_get_param_by_name(filter->effect, name);
        }
        filter->param_box_count = gs_effect_get_param_by_name(filter->effect, "box_count");
        filter->param_censor_mode = gs_effect_get_param_by_name(filter->effect, "censor_mode");
        filter->param_blur_radius = gs_effect_get_param_by_name(filter->effect, "blur_radius");
        filter->param_pixel_size = gs_effect_get_param_by_name(filter->effect, "pixel_size");
        filter->param_box_padding = gs_effect_get_param_by_name(filter->effect, "box_padding");
        filter->param_invert_y = gs_effect_get_param_by_name(filter->effect, "invert_y");
        filter->param_show_debug = gs_effect_get_param_by_name(filter->effect, "show_debug");
        filter->param_censor_all = gs_effect_get_param_by_name(filter->effect, "censor_all");
        filter->param_image_size = gs_effect_get_param_by_name(filter->effect, "image_size");
    }
    
    // Graphics render targets for AI video frame capture (640x360)
    filter->texrender = gs_texrender_create(GS_RGBA, 0);
    filter->stagesurf = gs_stagesurface_create(BLEWRED_SHM_WIDTH, BLEWRED_SHM_HEIGHT, GS_RGBA);
    obs_leave_graphics();
    
    // Initialize Shared Memory frame transmitter for BlewRed AI Core
    filter->shm_handle = CreateFileMappingW(INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE, 0,
                                            (DWORD)BLEWRED_SHM_TOTAL_SIZE, BLEWRED_SHM_NAME);
    if (filter->shm_handle) {
        filter->shm_ptr = MapViewOfFile(filter->shm_handle, FILE_MAP_ALL_ACCESS, 0, 0, BLEWRED_SHM_TOTAL_SIZE);
        if (filter->shm_ptr) {
            blewred_shm_header_t *hdr = (blewred_shm_header_t *)filter->shm_ptr;
            memset(hdr, 0, sizeof(blewred_shm_header_t));
            hdr->magic = 0x424C5752;
            hdr->version = 1;
            hdr->width = BLEWRED_SHM_WIDTH;
            hdr->height = BLEWRED_SHM_HEIGHT;
            hdr->stride = BLEWRED_SHM_WIDTH * 4;
            blog(LOG_INFO, "[BlewRed Filter] Shared Memory frame transmitter ready: %ux%u (%zu bytes)",
                 BLEWRED_SHM_WIDTH, BLEWRED_SHM_HEIGHT, (size_t)BLEWRED_SHM_TOTAL_SIZE);
        }
    }
    
    // Initialize Shared Singleton UDP Receiver
    shared_udp_init();
    
    obs_source_update(context, settings);
    return filter;
}

static void blewred_filter_destroy(void *data)
{
    blewred_filter_data_t *filter = (blewred_filter_data_t *)data;
    if (filter) {
        shared_udp_cleanup();
        
        obs_enter_graphics();
        if (filter->stagesurf) {
            gs_stagesurface_destroy(filter->stagesurf);
            filter->stagesurf = NULL;
        }
        if (filter->texrender) {
            gs_texrender_destroy(filter->texrender);
            filter->texrender = NULL;
        }
        if (filter->effect) {
            gs_effect_destroy(filter->effect);
            filter->effect = NULL;
        }
        obs_leave_graphics();
        
        if (filter->shm_ptr) {
            UnmapViewOfFile(filter->shm_ptr);
            filter->shm_ptr = NULL;
        }
        if (filter->shm_handle) {
            CloseHandle(filter->shm_handle);
            filter->shm_handle = NULL;
        }
        
        DeleteCriticalSection(&filter->cs);
        bfree(filter);
    }
}

static obs_properties_t *blewred_filter_properties(void *data)
{
    UNUSED_PARAMETER(data);
    obs_properties_t *props = obs_properties_create();
    
    obs_property_t *p_mode = obs_properties_add_list(props, "censor_mode", "Censor Mode",
                                                     OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);
    obs_property_list_add_int(p_mode, "Smart Defocus Blur", 0);
    obs_property_list_add_int(p_mode, "Mosaic / Pixelate", 1);
    obs_property_list_add_int(p_mode, "Solid Obsidian", 2);
    obs_property_list_add_int(p_mode, "Frosted Glass", 3);
    obs_property_list_add_int(p_mode, "Crimson Warning", 4);
    
    obs_properties_add_float_slider(props, "blur_radius", "Blur Radius", 4.0, 64.0, 1.0);
    obs_properties_add_float_slider(props, "pixel_size", "Pixel Size", 6.0, 64.0, 1.0);
    obs_properties_add_float_slider(props, "box_padding", "Box Margin %", 0.0, 50.0, 1.0);
    obs_properties_add_float_slider(props, "smoothing", "Smoothing %", 0.0, 90.0, 1.0);
    obs_properties_add_bool(props, "invert_y", "Invert Y Axis");
    obs_properties_add_bool(props, "show_debug", "Show AI Boxes");
    obs_properties_add_int(props, "udp_port", "blewred AI Sync Port", 1024, 65535, 1);
    
    return props;
}

static void blewred_filter_defaults(obs_data_t *settings)
{
    obs_data_set_default_int(settings, "censor_mode", 0);
    obs_data_set_default_double(settings, "blur_radius", 24.0);
    obs_data_set_default_double(settings, "pixel_size", 20.0);
    obs_data_set_default_double(settings, "box_padding", 15.0);
    obs_data_set_default_double(settings, "smoothing", 40.0);
    obs_data_set_default_bool(settings, "invert_y", false);
    obs_data_set_default_bool(settings, "show_debug", false);
    obs_data_set_default_int(settings, "udp_port", BLEWRED_DEFAULT_PORT);
}

struct obs_source_info blewred_filter_info = {
    .id = "blewred_filter",
    .type = OBS_SOURCE_TYPE_FILTER,
    .output_flags = OBS_SOURCE_VIDEO,
    .get_name = blewred_get_name,
    .create = blewred_filter_create,
    .destroy = blewred_filter_destroy,
    .video_tick = blewred_video_tick,
    .video_render = blewred_video_render,
    .get_properties = blewred_filter_properties,
    .get_defaults = blewred_filter_defaults,
    .update = blewred_filter_update,
};

MODULE_EXPORT uint32_t obs_module_ver(void)
{
    return 0x20020002;
}

MODULE_EXPORT void obs_module_set_pointer(obs_module_t *module)
{
    obs_current_module_ptr = module;
}

MODULE_EXPORT bool obs_module_load(void)
{
    obs_register_source(&blewred_filter_info);
    blog(LOG_INFO, "[BlewRed Filter Plugin v1.2.0] Initialized & registered with vector GPU blur & mosaic!");
    return true;
}

MODULE_EXPORT void obs_module_unload(void)
{
    blog(LOG_INFO, "[BlewRed Filter Plugin v1.2.0] Unloaded.");
}

MODULE_EXPORT const char *obs_module_name(void)
{
    return "BlewRed AI Smart Shield";
}

MODULE_EXPORT const char *obs_module_description(void)
{
    return "BlewRed AI Smart Censor Filter - Real-time Cascaded AI Blur & Shield for OBS Studio";
}

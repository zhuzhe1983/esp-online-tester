// Host-encoded JPEG through the same pinned public driver; AI-Thinker wiring.
// A common ESP32-CAM JPEG setup (20 MHz XCLK, quality 12, VGA then QVGA); C10_PSRAM
// selects its PSRAM variant: two framebuffers in external RAM.
#include <assert.h>
#include <stdio.h>
#include "esp_camera.h"
#include "esp_heap_caps.h"
#include "esp_memory_utils.h"
#include "mbedtls/sha256.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

#ifdef C10_PSRAM
#define FB_LOCATION CAMERA_FB_IN_PSRAM
#define FB_COUNT 2
#else
#define FB_LOCATION CAMERA_FB_IN_DRAM
#define FB_COUNT 1
#endif

static void init(pixformat_t format, framesize_t size, int fb_count) {
    const camera_config_t config = {
        .pin_pwdn = 32, .pin_reset = -1, .pin_xclk = 0,
        .pin_sccb_sda = 26, .pin_sccb_scl = 27,
        .pin_d0 = 5, .pin_d1 = 18, .pin_d2 = 19, .pin_d3 = 21,
        .pin_d4 = 36, .pin_d5 = 39, .pin_d6 = 34, .pin_d7 = 35,
        .pin_vsync = 25, .pin_href = 23, .pin_pclk = 22,
        .xclk_freq_hz = 20000000, .ledc_timer = LEDC_TIMER_0,
        .ledc_channel = LEDC_CHANNEL_0, .pixel_format = format,
        .frame_size = size, .jpeg_quality = 12, .fb_count = fb_count,
        .fb_location = FB_LOCATION, .grab_mode = CAMERA_GRAB_WHEN_EMPTY,
    };
    assert(esp_camera_init(&config) == ESP_OK);
}

static void frame(const char *phase, unsigned n, pixformat_t format, unsigned w, unsigned h) {
    camera_fb_t *fb = esp_camera_fb_get();
    assert(fb != NULL);
    assert(fb->format == format && fb->width == w && fb->height == h);
    unsigned char hash[32];
    assert(mbedtls_sha256(fb->buf, fb->len, hash, 0) == 0);
    printf("C10 %s frame=%u width=%u height=%u len=%u %s %s sha256=", phase, n,
           (unsigned)fb->width, (unsigned)fb->height, (unsigned)fb->len,
           format == PIXFORMAT_JPEG ? "JPEG" : "RGB565",
           esp_ptr_external_ram(fb->buf) ? "PSRAM" : "DRAM");
    for (unsigned i = 0; i < sizeof(hash); ++i) printf("%02x", hash[i]);
    printf("\n");
    esp_camera_fb_return(fb);
}

// The host acts on each marker while the guest waits; a frame already queued
// may predate the change, so it is printed but is not a freshness oracle.
static void marker(const char *text) {
    printf("C10 %s\n", text);
    fflush(stdout);
    vTaskDelay(pdMS_TO_TICKS(1000));
}

static void pair(const char *phase, pixformat_t format, unsigned w, unsigned h) {
    frame(phase, 0, format, w, h);
    frame(phase, 1, format, w, h);
}

void app_main(void) {
    printf("C10 psram=%u\n", (unsigned)heap_caps_get_total_size(MALLOC_CAP_SPIRAM));
    init(PIXFORMAT_JPEG, FRAMESIZE_VGA, FB_COUNT);
    printf("C10 init=ESP_OK JPEG VGA fb_count=%d\n", FB_COUNT);
    pair("vga-a", PIXFORMAT_JPEG, 640, 480);
    marker("REPLACE vga-b");
    frame("queued", 0, PIXFORMAT_JPEG, 640, 480);
    pair("vga-b", PIXFORMAT_JPEG, 640, 480);
    sensor_t *sensor = esp_camera_sensor_get();
    assert(sensor != NULL && sensor->set_framesize(sensor, FRAMESIZE_QVGA) == 0);
    marker("QVGA qvga-a");
    frame("queued", 0, PIXFORMAT_JPEG, 320, 240);
    pair("qvga-a", PIXFORMAT_JPEG, 320, 240);
    marker("REPLACE qvga-b");
    frame("queued", 0, PIXFORMAT_JPEG, 320, 240);
    pair("qvga-b", PIXFORMAT_JPEG, 320, 240);
    // Raw output in between: the stored host JPEG must not leak into it. One
    // framebuffer: raw capture with two is outside this fixture.
    assert(esp_camera_deinit() == ESP_OK);
    init(PIXFORMAT_RGB565, FRAMESIZE_QQVGA, 1);
    printf("C10 reinit=ESP_OK RGB565 QQVGA\n");
    pair("raw", PIXFORMAT_RGB565, 160, 120);
    assert(esp_camera_deinit() == ESP_OK);
    init(PIXFORMAT_JPEG, FRAMESIZE_QVGA, FB_COUNT);
    printf("C10 reinit=ESP_OK JPEG QVGA\n");
    pair("reinit", PIXFORMAT_JPEG, 320, 240);
    printf("C10 PASS\n");
    fflush(stdout);
    for (;;) vTaskDelay(pdMS_TO_TICKS(1000));
}

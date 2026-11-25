// SPDX-License-Identifier: MIT
// Independently authored diagnostic; not Waveshare factory firmware.
#include <errno.h>
#include <inttypes.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "bsp/esp-bsp.h"
#include "driver/gpio.h"
#include "esp_crc.h"
#include "esp_lv_adapter.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "lvgl.h"

typedef struct {
    const char *name;
    int gpio;
    uint32_t key;
    uint32_t down;
    uint32_t up;
    lv_obj_t *label;
} key_row_t;

static key_row_t keys[] = {
    {.name = "PREV", .gpio = 2, .key = LV_KEY_LEFT},
    {.name = "NEXT", .gpio = 3, .key = LV_KEY_RIGHT},
    {.name = "BOOT", .gpio = 28, .key = LV_KEY_ENTER},
};
static lv_indev_read_cb_t original_read_cb;
static lv_obj_t *recent_label;
static lv_obj_t *tf_label;
static TaskHandle_t main_task;
static bool pressed;
static bool ambiguous;
static uint32_t held_key;
static uint32_t event_sequence;

static key_row_t *row_for_key(uint32_t key)
{
    for (unsigned i = 0; i < sizeof(keys) / sizeof(keys[0]); ++i) {
        if (keys[i].key == key) return &keys[i];
    }
    return NULL;
}

static void report_edge(uint32_t key, bool down)
{
    key_row_t *row = row_for_key(key);
    if (!row) return;
    if (down) ++row->down;
    else ++row->up;
    ++event_sequence;
    lv_label_set_text_fmt(row->label, "%s GPIO%d   DOWN %" PRIu32 "  UP %" PRIu32,
                         row->name, row->gpio, row->down, row->up);
    lv_label_set_text_fmt(recent_label, "#%" PRIu32 " %s %s", event_sequence,
                         row->name, down ? "DOWN" : "UP");
    printf("C5D1 KEY seq=%" PRIu32 " key=%s gpio=%d state=%s down=%" PRIu32 " up=%" PRIu32 "\n",
           event_sequence, row->name, row->gpio, down ? "DOWN" : "UP", row->down, row->up);
    fflush(stdout);
    if (!down && row->gpio == 28) xTaskNotifyGive(main_task);
}

static void diagnostic_read(lv_indev_t *indev, lv_indev_data_t *data)
{
    // Keep the BSP's button debounce, GPIO driver and adapter projection intact.
    original_read_cb(indev, data);
    if (data->state == LV_INDEV_STATE_PRESSED) {
        if (!pressed) {
            pressed = true;
            ambiguous = false;
            held_key = data->key;
            report_edge(held_key, true);
        } else if (data->key != held_key && !ambiguous) {
            // The original adapter prioritizes PREV > NEXT > BOOT. Do not turn
            // a priority change into a fabricated physical release/press.
            ambiguous = true;
            lv_label_set_text(recent_label, "Multiple keys: release all, retry");
            printf("C5D1 INPUT_AMBIGUOUS held=%" PRIu32 " selected=%" PRIu32 "\n", held_key, data->key);
            fflush(stdout);
        }
    } else if (pressed) {
        if (!ambiguous) report_edge(held_key, false);
        pressed = false;
        ambiguous = false;
    }
}

static lv_obj_t *text_at(lv_obj_t *parent, int x, int y, int width, const char *text)
{
    lv_obj_t *label = lv_label_create(parent);
    lv_obj_set_pos(label, x, y);
    lv_obj_set_width(label, width);
    lv_label_set_text(label, text);
    return label;
}

static void make_interface(lv_indev_t *indev, bool mounted)
{
    lv_obj_t *screen = lv_screen_active();
    lv_obj_set_style_bg_color(screen, lv_color_hex(0x102033), 0);
    lv_obj_set_style_text_color(screen, lv_color_hex(0xffffff), 0);
    lv_obj_remove_flag(screen, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_t *title = text_at(screen, 16, 8, 296, "C5 LCD2.73");
    lv_obj_set_style_text_font(title, &lv_font_montserrat_24, 0);
    text_at(screen, 16, 38, 296, "Independent diagnostic / one key at a time");
    lv_group_t *group = lv_group_create();
    for (unsigned i = 0; i < sizeof(keys) / sizeof(keys[0]); ++i) {
        lv_obj_t *button = lv_button_create(screen);
        lv_obj_set_pos(button, 12, 64 + 52 * i);
        lv_obj_set_size(button, 296, 44);
        lv_obj_remove_flag(button, LV_OBJ_FLAG_SCROLLABLE);
        lv_obj_set_style_bg_color(button, lv_color_hex(0x263b53), 0);
        lv_obj_set_style_border_width(button, 3, LV_STATE_FOCUSED);
        lv_obj_set_style_border_color(button, lv_color_hex(0x59e0a0), LV_STATE_FOCUSED);
        lv_obj_set_style_bg_color(button, lv_color_hex(0x2463a6), LV_STATE_PRESSED);
        keys[i].label = lv_label_create(button);
        lv_label_set_text_fmt(keys[i].label, "%s GPIO%d   DOWN 0  UP 0", keys[i].name, keys[i].gpio);
        lv_obj_center(keys[i].label);
        lv_group_add_obj(group, button);
    }
    recent_label = text_at(screen, 16, 226, 296, "No key events yet");
    tf_label = text_at(screen, 16, 264, 296, mounted ? "TF mounted / read pending" : "TF mount FAILED");
    lv_indev_set_group(indev, group);
    original_read_cb = lv_indev_get_read_cb(indev);
    ESP_ERROR_CHECK(original_read_cb ? ESP_OK : ESP_FAIL);
    lv_indev_set_read_cb(indev, diagnostic_read);
}

static void read_existing_file(void)
{
    static const char expected[] = "ESP32-C5 TF card read/write test\r\nWrite and read verification passed.\r\n";
    char status[96];
    unsigned char data[128];
    FILE *file = fopen(BSP_SD_MOUNT_POINT "/test.txt", "rb");
    if (!file) {
        int error = errno;
        snprintf(status, sizeof(status), error == ENOENT ? "TF mounted / test.txt absent (%d)" : "TF mounted / open error (%d)", error);
        printf("C5D1 TF READ status=%s errno=%d\n", error == ENOENT ? "absent" : "open_error", error);
    } else {
        size_t size = fread(data, 1, sizeof(data), file);
        int next = fgetc(file);
        bool error = ferror(file) != 0;
        int closed = fclose(file);
        bool eof = next == EOF;
        bool match = !error && closed == 0 && eof && size == sizeof(expected) - 1 &&
                     memcmp(data, expected, size) == 0;
        uint32_t crc = esp_crc32_le(0, data, size);
        snprintf(status, sizeof(status), "TF read %uB / %s", (unsigned)size,
                 error || closed ? "ERROR" : match ? "71B MATCH" : "other content");
        printf("C5D1 TF READ status=%s prefix_bytes=%u eof=%u crc32_le=%08" PRIx32 "\n",
               error || closed ? "read_error" : match ? "match" : "other", (unsigned)size, eof, crc);
    }
    fflush(stdout);
    ESP_ERROR_CHECK(esp_lv_adapter_lock(-1));
    lv_label_set_text(tf_label, status);
    esp_lv_adapter_unlock();
}

static void select_idle(int gpio)
{
    ESP_ERROR_CHECK(gpio_reset_pin(gpio));
    ESP_ERROR_CHECK(gpio_set_level(gpio, 1));
    ESP_ERROR_CHECK(gpio_set_direction(gpio, GPIO_MODE_OUTPUT));
}

void app_main(void)
{
    main_task = xTaskGetCurrentTaskHandle();
    printf("C5D1 START app=c5_lcd273_diagnostic version=0.1.0\n");
    // IDF553 sdspi_share.rst: initialize shared bus, idle other CS, mount the
    // card into SPI mode before communicating with other SPI devices.
    select_idle(BSP_LCD_CS);
    select_idle(BSP_SD_SPI_CS);
    ESP_ERROR_CHECK(bsp_spi_init(BSP_LCD_H_RES * BSP_LCD_DRAW_BUFFER_HEIGHT * 3));
    esp_err_t mounted = bsp_sdcard_mount();
    printf("C5D1 TF MOUNT result=%s code=%d format=0 app_writes=0\n", esp_err_to_name(mounted), mounted);
    ESP_ERROR_CHECK(bsp_io_expander_init());
    IO_EXTENSION_Output(IO_EXTENSION_IO_0, 0);
    vTaskDelay(pdMS_TO_TICKS(10));
    IO_EXTENSION_Output(IO_EXTENSION_IO_0, 1);
    lv_display_t *display = bsp_display_start();
    ESP_ERROR_CHECK(display ? ESP_OK : ESP_FAIL);
    ESP_ERROR_CHECK(bsp_display_backlight_on());
    ESP_ERROR_CHECK(esp_lv_adapter_lock(-1));
    lv_indev_t *indev = bsp_display_get_input_dev();
    ESP_ERROR_CHECK(indev ? ESP_OK : ESP_FAIL);
    make_interface(indev, mounted == ESP_OK);
    esp_lv_adapter_unlock();
    printf("C5D1 READY pins=2,3,28 active=0 single_key=1 tf=%s\n", mounted == ESP_OK ? "mounted" : "failed");
    fflush(stdout);
    vTaskDelay(pdMS_TO_TICKS(200));
    if (mounted == ESP_OK) read_existing_file();
    for (;;) {
        ulTaskNotifyTake(pdTRUE, portMAX_DELAY);
        if (mounted == ESP_OK) read_existing_file();
    }
}

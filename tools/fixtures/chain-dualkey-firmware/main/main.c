#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

#include "driver/gpio.h"
#include "driver/rmt_encoder.h"
#include "driver/rmt_tx.h"
#include "esp_err.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

enum {
    KEY1 = GPIO_NUM_0,
    KEY2 = GPIO_NUM_17,
    RGB_DATA = GPIO_NUM_21,
    RGB_POWER = GPIO_NUM_40,
};

void app_main(void)
{
    const gpio_config_t keys = {
        .pin_bit_mask = (1ULL << KEY1) | (1ULL << KEY2),
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
    };
    const gpio_config_t power = {
        .pin_bit_mask = 1ULL << RGB_POWER,
        .mode = GPIO_MODE_OUTPUT_OD,
    };
    ESP_ERROR_CHECK(gpio_config(&keys));
    ESP_ERROR_CHECK(gpio_config(&power));
    ESP_ERROR_CHECK(gpio_set_level(RGB_POWER, 0));

    const rmt_tx_channel_config_t tx_config = {
        .gpio_num = RGB_DATA,
        .clk_src = RMT_CLK_SRC_DEFAULT,
        .resolution_hz = 10000000,
        .mem_block_symbols = 64,
        .trans_queue_depth = 1,
    };
    const rmt_bytes_encoder_config_t grb = {
        .bit0 = { .level0 = 1, .duration0 = 3, .level1 = 0, .duration1 = 9 },
        .bit1 = { .level0 = 1, .duration0 = 9, .level1 = 0, .duration1 = 3 },
        .flags.msb_first = true,
    };
    rmt_channel_handle_t channel = NULL;
    rmt_encoder_handle_t encoder = NULL;
    ESP_ERROR_CHECK(rmt_new_tx_channel(&tx_config, &channel));
    ESP_ERROR_CHECK(rmt_new_bytes_encoder(&grb, &encoder));
    ESP_ERROR_CHECK(rmt_enable(channel));

    puts("C147 READY");
    int previous = -1;
    while (true) {
        const bool key1 = gpio_get_level(KEY1) == 0;
        const bool key2 = gpio_get_level(KEY2) == 0;
        const int state = key1 | (key2 << 1);
        if (state != previous) {
            // Wire order from the C147 schematic. GRB, KEY2 before KEY1.
            const uint8_t pixels[] = { key2 ? 191 : 0, 0, 0, 0, key1 ? 127 : 0, 0 };
            const rmt_transmit_config_t transmit = { .loop_count = 0 };
            ESP_ERROR_CHECK(rmt_transmit(channel, encoder, pixels, sizeof(pixels), &transmit));
            ESP_ERROR_CHECK(rmt_tx_wait_all_done(channel, 100));
            printf("C147 KEYS %d %d\n", key1, key2);
            previous = state;
        }
        vTaskDelay(pdMS_TO_TICKS(20));
    }
}

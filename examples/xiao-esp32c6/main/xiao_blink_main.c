// The Seeed Studio XIAO ESP32-C6 blink firmware: the user LED on GPIO 15 and the BOOT button
// on GPIO 9. Build it with ESP-IDF 5.x:
//
//   idf.py set-target esp32c6 build
//
// and run it on the emulator with examples/xiao-esp32c6/run.sh.
#include <stdio.h>

#include "driver/gpio.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

// The user LED is low active: the anode goes to 3V3 through a resistor, the cathode to the pin,
// so a low pin is a lit LED. The BOOT button is on GPIO 9, active low and pulled up.
#define PIN_LED 15
#define PIN_BOOT 9

static const char *TAG = "xiao";

void app_main(void)
{
    gpio_config_t led = {
        .pin_bit_mask = 1ULL << PIN_LED,
        .mode = GPIO_MODE_OUTPUT,
        .pull_up_en = GPIO_PULLUP_DISABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_DISABLE,
    };
    ESP_ERROR_CHECK(gpio_config(&led));

    gpio_config_t boot = {
        .pin_bit_mask = 1ULL << PIN_BOOT,
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_DISABLE,
    };
    ESP_ERROR_CHECK(gpio_config(&boot));

    ESP_LOGI(TAG, "XIAO ESP32-C6: user LED on GPIO %d (low active), BOOT button on GPIO %d", PIN_LED, PIN_BOOT);

    int level = 0;
    while (1) {
        // level 0 lights the LED: the pin sinks the current
        gpio_set_level(PIN_LED, level);
        printf("LED %s, BOOT button %s\n", level ? "off" : "on",
               gpio_get_level(PIN_BOOT) ? "released" : "pressed");
        fflush(stdout);
        level = !level;
        vTaskDelay(pdMS_TO_TICKS(500));
    }
}

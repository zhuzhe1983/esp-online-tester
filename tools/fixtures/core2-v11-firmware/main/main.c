#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include "driver/gpio.h"
#include "driver/i2c_master.h"
#include "driver/spi_master.h"
#include "esp_check.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static i2c_master_dev_handle_t add_device(i2c_master_bus_handle_t bus, uint16_t address)
{
    i2c_device_config_t config = {.device_address = address, .scl_speed_hz = 100000};
    i2c_master_dev_handle_t device;
    ESP_ERROR_CHECK(i2c_master_bus_add_device(bus, &config, &device));
    return device;
}

static void send_lcd(spi_device_handle_t lcd, bool data, const uint8_t *bytes, size_t count)
{
    ESP_ERROR_CHECK(gpio_set_level(GPIO_NUM_15, data));
    spi_transaction_t tx = {.length = count * 8, .tx_buffer = bytes};
    ESP_ERROR_CHECK(spi_device_polling_transmit(lcd, &tx));
}

void app_main(void)
{
    const i2c_master_bus_config_t i2c = {
        .i2c_port = I2C_NUM_1, .sda_io_num = GPIO_NUM_21, .scl_io_num = GPIO_NUM_22,
        .clk_source = I2C_CLK_SRC_DEFAULT, .glitch_ignore_cnt = 7,
    };
    i2c_master_bus_handle_t bus;
    ESP_ERROR_CHECK(i2c_new_master_bus(&i2c, &bus));
    i2c_master_dev_handle_t power = add_device(bus, 0x34);
    i2c_master_dev_handle_t touch = add_device(bus, 0x38);
    i2c_master_dev_handle_t meter = add_device(bus, 0x40);
    uint8_t id;
    ESP_ERROR_CHECK(i2c_master_transmit_receive(power, (const uint8_t[]){3}, 1, &id, 1, 100));
    assert(id == 0x4a);

    // Core2 v1.1 schematic and M5GFX pmic_ops::power2101. Turn on the logic,
    // release ALDO2 reset, then enable BLDO1 from the same register.
    ESP_ERROR_CHECK(i2c_master_transmit(power, (const uint8_t[]){0x90, 0x08}, 2, 100));
    vTaskDelay(pdMS_TO_TICKS(20));
    ESP_ERROR_CHECK(i2c_master_transmit(power, (const uint8_t[]){0x90, 0x1a}, 2, 100));
    ESP_ERROR_CHECK(i2c_master_transmit(touch, (const uint8_t[]){0xa4, 0}, 2, 100));
    uint8_t ina[2];
    ESP_ERROR_CHECK(i2c_master_transmit_receive(meter, (const uint8_t[]){0xff}, 1, ina, 2, 100));
    assert(ina[0] == 0x32 && ina[1] == 0x20);

    const spi_bus_config_t spi = {
        .sclk_io_num = GPIO_NUM_18, .mosi_io_num = GPIO_NUM_23, .miso_io_num = GPIO_NUM_38,
        .quadwp_io_num = -1, .quadhd_io_num = -1,
    };
    const spi_device_interface_config_t panel = {
        .clock_speed_hz = 10000000, .spics_io_num = GPIO_NUM_5, .queue_size = 1,
    };
    spi_device_handle_t lcd;
    ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST, &spi, SPI_DMA_DISABLED));
    ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST, &panel, &lcd));
    const gpio_config_t dc = {.pin_bit_mask = 1ULL << GPIO_NUM_15, .mode = GPIO_MODE_OUTPUT};
    const gpio_config_t irq = {.pin_bit_mask = 1ULL << GPIO_NUM_39, .mode = GPIO_MODE_INPUT};
    ESP_ERROR_CHECK(gpio_config(&dc));
    ESP_ERROR_CHECK(gpio_config(&irq));
    send_lcd(lcd, false, (const uint8_t[]){0x11}, 1);
    vTaskDelay(pdMS_TO_TICKS(120));
    send_lcd(lcd, false, (const uint8_t[]){0x29}, 1);
    send_lcd(lcd, false, (const uint8_t[]){0x3a}, 1);
    send_lcd(lcd, true, (const uint8_t[]){0x55}, 1);
    send_lcd(lcd, false, (const uint8_t[]){0x2c}, 1);
    send_lcd(lcd, true, (const uint8_t[]){0xf8, 0x00, 0x07, 0xe0, 0x00, 0x1f}, 6);
    puts("CORE2 V11 AXP2101=4A INA3221=3220 LCD ready");
    for (;;) {
        uint8_t point[5];
        ESP_ERROR_CHECK(i2c_master_transmit_receive(touch, (const uint8_t[]){2}, 1, point, 5, 100));
        if (point[0] && gpio_get_level(GPIO_NUM_39) == 0) {
            const int x = (point[1] & 15) * 256 + point[2];
            const int y = (point[3] & 15) * 256 + point[4];
            printf("CORE2 V11 TOUCH %d %d\n", x, y);
        }
        vTaskDelay(pdMS_TO_TICKS(80));
    }
}

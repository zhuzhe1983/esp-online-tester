// Diagnostic firmware for K150. Pins come from the official StickS3 product
// page and M5GFX 65c816a3 (StickS3 branch, lines 2995-3063).
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "driver/gpio.h"
#include "driver/i2c.h"
#include "driver/spi_master.h"
#include "esp_psram.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "../../atoms3r-firmware/main/bmi270_config.h"

enum { PM1 = 0x6e, BMI270 = 0x68, ES8311 = 0x18, LCD_DC = 45 };
static spi_device_handle_t lcd;

static void write_register(uint8_t address, uint8_t reg, uint8_t value) {
    uint8_t bytes[] = {reg, value};
    ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0, address, bytes, sizeof bytes, pdMS_TO_TICKS(100)));
}

static void read_registers(uint8_t address, uint8_t reg, uint8_t *bytes, size_t count) {
    ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_0, address, &reg, 1, bytes, count, pdMS_TO_TICKS(100)));
}

static uint8_t read_register(uint8_t address, uint8_t reg) {
    uint8_t result;
    read_registers(address, reg, &result, 1);
    return result;
}

static void transfer(bool data, const uint8_t *bytes, size_t count) {
    ESP_ERROR_CHECK(gpio_set_level(LCD_DC, data));
    spi_transaction_t transaction = {.length = count * 8, .tx_buffer = bytes};
    ESP_ERROR_CHECK(spi_device_polling_transmit(lcd, &transaction));
}

static void command(uint8_t cmd, const uint8_t *data, size_t count) {
    transfer(false, &cmd, 1);
    if (count) transfer(true, data, count);
}

static void display_color(uint16_t color) {
    // Centered 32 × 32 square in the 135 × 240 window at controller (52,40).
    command(0x2a, (uint8_t[]){0, 103, 0, 134}, 4);
    command(0x2b, (uint8_t[]){0, 144, 0, 175}, 4);
    command(0x2c, NULL, 0);
    uint8_t row[64];
    for (size_t i = 0; i < sizeof row; i += 2) {
        row[i] = color >> 8;
        row[i + 1] = color & 255;
    }
    for (int line = 0; line < 32; line++) transfer(true, row, sizeof row);
}

static void init_display(void) {
    // PM1 GPIO2 in GPIO mode, push-pull output high supplies the LCD L3B rail.
    write_register(PM1, 0x16, read_register(PM1, 0x16) & ~(3 << 4));
    write_register(PM1, 0x13, read_register(PM1, 0x13) & ~(1 << 2));
    write_register(PM1, 0x10, read_register(PM1, 0x10) | (1 << 2));
    write_register(PM1, 0x11, read_register(PM1, 0x11) | (1 << 2));
    gpio_config_t outputs = {.pin_bit_mask = (1ULL << LCD_DC) | (1ULL << 21) | (1ULL << 38), .mode = GPIO_MODE_OUTPUT};
    ESP_ERROR_CHECK(gpio_config(&outputs));
    ESP_ERROR_CHECK(gpio_set_level(21, 0));
    vTaskDelay(pdMS_TO_TICKS(20));
    ESP_ERROR_CHECK(gpio_set_level(21, 1));
    spi_bus_config_t bus = {.mosi_io_num = 39, .miso_io_num = -1, .sclk_io_num = 40, .quadwp_io_num = -1, .quadhd_io_num = -1};
    ESP_ERROR_CHECK(spi_bus_initialize(SPI3_HOST, &bus, SPI_DMA_DISABLED));
    spi_device_interface_config_t panel = {.clock_speed_hz = 1000000, .spics_io_num = 41, .queue_size = 1, .flags = SPI_DEVICE_HALFDUPLEX | SPI_DEVICE_3WIRE};
    ESP_ERROR_CHECK(spi_bus_add_device(SPI3_HOST, &panel, &lcd));
    command(0x11, NULL, 0);
    vTaskDelay(pdMS_TO_TICKS(120));
    command(0x3a, (uint8_t[]){0x55}, 1);
    command(0x29, NULL, 0);
    ESP_ERROR_CHECK(gpio_set_level(38, 1));
}

static void init_imu(void) {
    write_register(BMI270, 0x7e, 0xb6);
    write_register(BMI270, 0x7c, 0);
    write_register(BMI270, 0x59, 0);
    for (size_t offset = 0; offset < sizeof bmi270_config_file; offset += 64) {
        write_register(BMI270, 0x5b, (offset / 2) & 15);
        write_register(BMI270, 0x5c, (offset / 2) >> 4);
        uint8_t data[65] = {0x5e};
        memcpy(data + 1, bmi270_config_file + offset, sizeof data - 1);
        ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0, BMI270, data, sizeof data, pdMS_TO_TICKS(100)));
    }
    write_register(BMI270, 0x59, 1);
    printf("STICKS3 BMI_INIT %02x\n", read_register(BMI270, 0x21));
    write_register(BMI270, 0x41, 2);
    write_register(BMI270, 0x7d, 4);
}

void app_main(void) {
    printf("STICKS3 PSRAM %u\n", (unsigned)esp_psram_get_size());
    i2c_config_t i2c = {.mode = I2C_MODE_MASTER, .sda_io_num = 47, .scl_io_num = 48,
                        .sda_pullup_en = GPIO_PULLUP_ENABLE, .scl_pullup_en = GPIO_PULLUP_ENABLE,
                        .master.clk_speed = 100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0, &i2c));
    ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0, I2C_MODE_MASTER, 0, 0, 0));
    printf("STICKS3 IDS PM1=%02x BMI270=%02x ES8311=%02x/%02x\n",
           read_register(PM1, 0), read_register(BMI270, 0), read_register(ES8311, 0xfd), read_register(ES8311, 0xfe));
    init_display();
    init_imu();
    gpio_config_t buttons = {.pin_bit_mask = (1ULL << 11) | (1ULL << 12),
                             .mode = GPIO_MODE_INPUT, .pull_up_en = GPIO_PULLUP_ENABLE};
    ESP_ERROR_CHECK(gpio_config(&buttons));
    const uint16_t colors[] = {0xffff, 0xf800, 0x07e0, 0x001f};
    int previous_buttons = -1, previous_x = INT32_MIN;
    for (;;) {
        int pressed = !gpio_get_level(11) + 2 * !gpio_get_level(12);
        if (pressed != previous_buttons) {
            display_color(colors[pressed]);
            printf("STICKS3 BUTTONS %d COLOR %04x\n", pressed, colors[pressed]);
            previous_buttons = pressed;
        }
        uint8_t sample[2];
        read_registers(BMI270, 0x0c, sample, sizeof sample);
        int x = (int16_t)((sample[1] << 8) | sample[0]);
        if (x != previous_x) {
            printf("STICKS3 ACCEL_X %d\n", x);
            previous_x = x;
        }
        fflush(stdout);
        vTaskDelay(pdMS_TO_TICKS(50));
    }
}

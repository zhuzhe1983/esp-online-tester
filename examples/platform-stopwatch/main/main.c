// SPDX-License-Identifier: MIT
// StopWatch diagnostic firmware. ESP-IDF v5.5.1 public drivers; no host drawing.
// Pinout: https://docs.m5stack.com/en/core/StopWatch (C152).
// PM1/IOE/CO5300 initialization follows pinned manufacturer sources in README.md.
// This fixture validates the model; it has not been qualified on physical hardware.
#include <stdio.h>
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/spi_master.h"
#include "driver/gpio.h"
#include "driver/uart.h"
#include "esp_timer.h"
#include "esp_log.h"
#include "esp_rom_sys.h"

static spi_device_handle_t lcd;
static uint16_t line[468];
static unsigned elapsed, gps_lines;
static bool ticking;
static const char *TAG = "stopwatch-fixture";

// M5PM1's default is 100 kHz. A sleeping controller may NACK this wake read.
// M5PM1 315d2899: src/M5PM1_i2c_compat.h (wake and 500 us transfer interval),
// src/M5PM1.cpp (bounded retries with 50 ms between failures).
static void wake_device(uint8_t addr) {
    uint8_t reg = 0, value = 0;
    esp_err_t err = i2c_master_write_read_device(I2C_NUM_0, addr, &reg, 1,
                                               &value, 1, pdMS_TO_TICKS(20));
    ESP_LOGI(TAG, "I2C wake addr=0x%02x: %s", addr, esp_err_to_name(err));
    vTaskDelay(pdMS_TO_TICKS(10));
}
static esp_err_t read_reg(uint8_t addr, uint8_t reg, uint8_t *value) {
    esp_err_t err = ESP_FAIL;
    for (int attempt = 0; attempt < 3; ++attempt) {
        err = i2c_master_write_read_device(I2C_NUM_0, addr, &reg, 1,
                                          value, 1, pdMS_TO_TICKS(100));
        esp_rom_delay_us(500);
        if (err == ESP_OK) return err;
        ESP_LOGW(TAG, "I2C read addr=0x%02x reg=0x%02x attempt=%d: %s",
                 addr, reg, attempt + 1, esp_err_to_name(err));
        if (attempt < 2) vTaskDelay(pdMS_TO_TICKS(50));
    }
    return err;
}
static void wr(uint8_t addr, uint8_t reg, uint8_t value) {
    uint8_t data[] = {reg, value};
    esp_err_t err = ESP_FAIL;
    for (int attempt = 0; attempt < 3; ++attempt) {
        err = i2c_master_write_to_device(I2C_NUM_0, addr, data, 2, pdMS_TO_TICKS(100));
        esp_rom_delay_us(500);
        if (err == ESP_OK) return;
        ESP_LOGW(TAG, "I2C write addr=0x%02x reg=0x%02x value=0x%02x attempt=%d: %s",
                 addr, reg, value, attempt + 1, esp_err_to_name(err));
        if (attempt < 2) vTaskDelay(pdMS_TO_TICKS(50));
    }
    ESP_ERROR_CHECK(err);
}
static uint8_t rd(uint8_t addr, uint8_t reg) {
    uint8_t value = 0;
    ESP_ERROR_CHECK(read_reg(addr, reg, &value));
    return value;
}
static void update(uint8_t addr, uint8_t reg, uint8_t set, uint8_t clear) {
    wr(addr, reg, (rd(addr, reg) & ~clear) | set);
}
static void command(uint8_t reg, const void *data, size_t size, bool quad) {
    spi_transaction_t tx = {
        .cmd = quad ? 0x32 : 0x02, .addr = (uint32_t)reg << 8,
        .length = size * 8, .tx_buffer = data,
        .flags = quad ? SPI_TRANS_MODE_QIO : 0,
    };
    ESP_ERROR_CHECK(spi_device_polling_transmit(lcd, &tx));
}
static void rect(int x, int y, int w, int h, uint16_t color) {
    uint16_t be = (color << 8) | (color >> 8);
    for (int i = 0; i < w; i++) line[i] = be;
    int x0 = x + 6, x1 = x0 + w - 1, y1 = y + h - 1;
    uint8_t cols[] = {x0 >> 8, x0, x1 >> 8, x1};
    uint8_t rows[] = {y >> 8, y, y1 >> 8, y1};
    command(0x2a, cols, sizeof(cols), false);
    command(0x2b, rows, sizeof(rows), false);
    for (int j = 0; j < h; j++) command(j ? 0x3c : 0x2c, line, w * 2, true);
}
static void digit(int x, int y, int value, int scale, uint16_t color) {
    static const uint8_t glyph[10][7] = {
        {14,17,19,21,25,17,14}, {4,12,4,4,4,4,14}, {14,17,1,2,4,8,31},
        {30,1,1,14,1,1,30}, {2,6,10,18,31,2,2}, {31,16,16,30,1,1,30},
        {14,16,16,30,17,17,14}, {31,1,2,4,8,8,8}, {14,17,17,14,17,17,14},
        {14,17,17,15,1,1,14},
    };
    for (int r = 0; r < 7; r++) for (int c = 0; c < 5; c++)
        rect(x+c*scale,y+r*scale,scale,scale,(glyph[value%10][r] & (1 << (4-c))) ? color : 0x0842);
}
static void render(void) {
    unsigned seconds = elapsed % 6000;
    digit(57,175,seconds/600,12,0xff9c);
    digit(129,175,seconds/60%10,12,0xff9c);
    rect(204,193,9,9,0x07fb); rect(204,229,9,9,0x07fb);
    digit(228,175,seconds/10%6,12,0xff9c);
    digit(300,175,seconds%10,12,0xff9c);
    rect(150,300,168,8,ticking ? 0x07fb : 0xfc40);
    digit(219,339,gps_lines%10,5,gps_lines ? 0x07e0 : 0x4208);
}
void app_main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    i2c_config_t bus = {.mode=I2C_MODE_MASTER,.sda_io_num=47,.scl_io_num=48,
        .sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));
    ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    wake_device(0x6e);
    uint8_t pm1_id = 0;
    esp_err_t pm1_err = read_reg(0x6e,0x00,&pm1_id);
    if (pm1_err != ESP_OK) {
        vTaskDelay(pdMS_TO_TICKS(800));
        wake_device(0x6e);
        pm1_err = read_reg(0x6e,0x00,&pm1_id);
    }
    ESP_ERROR_CHECK(pm1_err);
    wr(0x6e,0x09,0); wr(0x6e,0x0a,0); // 100 kHz, no I2C sleep or PM1 watchdog
    update(0x6e,0x06,0x17,0); // LED_EN, LDO, DCDC and charging; preserve other bits
    // Never access BTN_CFG_1 (0x49): its bit 7 locks manual download entry.
    // StopWatch UserDemo 81099c78 hal_ioe.cpp: pin names are one-based.
    // P1 MUX=0, P3 audio=1, P4 touch/P5 OLED reset=1, P8 L3B supply=1;
    // P9 motor/P10 speaker amplifier=0. Only the selected output bits change.
    wake_device(0x4f); wr(0x4f,0x23,0);
    update(0x4f,0x05,0x84,0x19); update(0x4f,0x06,0,3);
    update(0x4f,0x09,0,0x9d); update(0x4f,0x0a,0,3);
    update(0x4f,0x0b,0,0x9d); update(0x4f,0x0c,0,3);
    update(0x4f,0x13,0,0x9d); update(0x4f,0x14,0,3);
    update(0x4f,0x03,0x9d,0); update(0x4f,0x04,3,0);
    vTaskDelay(pdMS_TO_TICKS(20)); update(0x4f,0x05,0x18,0);
    vTaskDelay(pdMS_TO_TICKS(80));
    ESP_LOGI(TAG, "100 kHz PM1 id=0x%02x power=0x%02x IOE output=0x%02x",
             pm1_id, rd(0x6e,0x06), rd(0x4f,0x05));
    spi_bus_config_t spi = {.sclk_io_num=40,.mosi_io_num=41,.miso_io_num=42,
        .quadwp_io_num=46,.quadhd_io_num=45,.max_transfer_sz=1024};
    ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST,&spi,SPI_DMA_CH_AUTO));
    spi_device_interface_config_t dev = {.command_bits=8,.address_bits=24,.clock_speed_hz=20000000,
        .mode=0,.spics_io_num=39,.queue_size=1,.flags=SPI_DEVICE_HALFDUPLEX};
    ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&dev,&lcd));
    // StopWatch UserDemo 81099c78 hal_display.cpp CO5300 initialization.
    command(0x11,NULL,0,false); vTaskDelay(pdMS_TO_TICKS(150));
    const uint8_t spi_mode=0x80, tear_mode=0x80, tear_line[]={0x01,0xd2};
    const uint8_t control=0x20, rotation=0, format=0x55, brightness=0xa0;
    command(0xc4,&spi_mode,1,false); command(0x35,&tear_mode,1,false);
    command(0x44,tear_line,sizeof(tear_line),false); command(0x53,&control,1,false);
    command(0x20,NULL,0,false); command(0x36,&rotation,1,false);
    command(0x3a,&format,1,false); command(0x51,&brightness,1,false); command(0x29,NULL,0,false);
    rect(0,0,468,466,0x0842); rect(110,115,248,4,0x07fb); render();
    gpio_config_t buttons={.pin_bit_mask=(1ULL<<1)|(1ULL<<2),.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};
    ESP_ERROR_CHECK(gpio_config(&buttons));
    uart_config_t uart={.baud_rate=9600,.data_bits=UART_DATA_8_BITS,.parity=UART_PARITY_DISABLE,
        .stop_bits=UART_STOP_BITS_1,.flow_ctrl=UART_HW_FLOWCTRL_DISABLE,.source_clk=UART_SCLK_DEFAULT};
    ESP_ERROR_CHECK(uart_param_config(UART_NUM_1,&uart));
    ESP_ERROR_CHECK(uart_set_pin(UART_NUM_1,10,11,UART_PIN_NO_CHANGE,UART_PIN_NO_CHANGE));
    ESP_ERROR_CHECK(uart_driver_install(UART_NUM_1,2048,0,0,NULL,0));
    printf("PLATFORM_READY stopwatch; A=start/pause B=reset; GPS=UART1 RX11 TX10 9600\n");
    int last_a=1,last_b=1,last_touch=0;
    int64_t last_tick=esp_timer_get_time();
    char nmea[256]; size_t used=0;
    while (true) {
        bool dirty=false;
        int a=gpio_get_level(2), b=gpio_get_level(1);
        if (!a && last_a) {ticking=!ticking; printf("BUTTON_A %s\n",ticking?"running":"paused");dirty=true;last_tick=esp_timer_get_time();}
        if (!b && last_b) {elapsed=0; printf("BUTTON_B reset\n");dirty=true;}
        last_a=a;last_b=b;
        int touch=rd(0x15,0x02)&0x0f;
        if (touch && !last_touch) {printf("TOUCH down\n");}
        last_touch=touch;
        uint8_t bytes[128]; int count=uart_read_bytes(UART_NUM_1,bytes,sizeof(bytes),0);
        for (int i=0;i<count;i++) {
            if (bytes[i]=='\n') {nmea[used]=0;printf("GPS_RX %s\n",nmea);used=0;gps_lines++;dirty=true;}
            else if (bytes[i]!='\r') {if(used<sizeof(nmea)-1)nmea[used++]=bytes[i];else used=0;}
        }
        int64_t now=esp_timer_get_time();
        if (now-last_tick>=1000000) {last_tick=now;if(ticking){elapsed++;dirty=true;}}
        if (dirty) render();
        vTaskDelay(pdMS_TO_TICKS(20));
    }
}

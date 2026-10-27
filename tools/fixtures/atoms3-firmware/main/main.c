// Internal ESP-IDF 5.5.1 protocol fixture, not a product firmware or Pages asset.
// Pin map: M5Stack AtomS3 C123 official docs. Driver constants: M5GFX 65c816a3,
// M5Unified 368ad608. Tests actual guest SPI, I2C and GPIO through the WASM worker.
#include <stdio.h>
#include <stdint.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "driver/i2c.h"

static spi_device_handle_t lcd;
static void transfer(int dc, const void *bytes, size_t n) {
    gpio_set_level(33, dc);
    spi_transaction_t t = {.length=n*8, .tx_buffer=bytes};
    ESP_ERROR_CHECK(spi_device_polling_transmit(lcd,&t));
}
static void command(uint8_t command, const uint8_t *data, size_t n) {transfer(0,&command,1);if(n)transfer(1,data,n);}
static void reg_write(uint8_t reg,uint8_t value) {uint8_t bytes[]={reg,value};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x68,bytes,2,pdMS_TO_TICKS(100)));}
static void reg_read(uint8_t reg,uint8_t *bytes,size_t n) {ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_0,0x68,&reg,1,bytes,n,pdMS_TO_TICKS(100)));}
void app_main(void) {
    gpio_config_t outputs={.pin_bit_mask=(1ULL<<33)|(1ULL<<34)|(1ULL<<16),.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&outputs));
    gpio_set_level(34,0);vTaskDelay(pdMS_TO_TICKS(10));gpio_set_level(34,1);gpio_set_level(16,1);
    gpio_config_t button={.pin_bit_mask=1ULL<<41,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&button));
    spi_bus_config_t bus={.mosi_io_num=21,.miso_io_num=-1,.sclk_io_num=17,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=256};
    ESP_ERROR_CHECK(spi_bus_initialize(SPI3_HOST,&bus,SPI_DMA_DISABLED));
    spi_device_interface_config_t panel={.clock_speed_hz=1000000,.spics_io_num=15,.queue_size=1};ESP_ERROR_CHECK(spi_bus_add_device(SPI3_HOST,&panel,&lcd));
    command(0x11,NULL,0);command(0x29,NULL,0);command(0x3a,(uint8_t[]){0x55},1);command(0x36,(uint8_t[]){0},1);
    command(0x2a,(uint8_t[]){0,0,0,127},4);command(0x2b,(uint8_t[]){0,32,0,159},4);command(0x2c,NULL,0);
    uint8_t line[256];for(int y=0;y<128;y++){for(int x=0;x<128;x++){uint16_t c=x<64?0xf800:0x07e0;line[x*2]=c>>8;line[x*2+1]=c;}for(int at=0;at<256;at+=64)transfer(1,line+at,64);}
    puts("ATOMS3_LCD_READY 128x128 red-left green-right");
    i2c_config_t imu={.mode=I2C_MODE_MASTER,.sda_io_num=38,.scl_io_num=39,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&imu));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));uint8_t id;reg_read(0x75,&id,1);printf("ATOMS3_IMU_ID %02x\n",id);
    reg_write(0x6b,0x80);reg_write(0x6b,1);reg_write(0x1c,0x10);reg_write(0x1b,0x18);
    int last=-1;for(;;){int b=gpio_get_level(41);if(b!=last){printf("ATOMS3_BUTTON %d\n",b);last=b;}
        uint8_t sample[14];reg_read(0x3b,sample,14);printf("ATOMS3_ACCEL %d %d %d\n",(int16_t)((sample[0]<<8)|sample[1]),(int16_t)((sample[2]<<8)|sample[3]),(int16_t)((sample[4]<<8)|sample[5]));printf("ATOMS3_GYRO %d %d %d\n",(int16_t)((sample[8]<<8)|sample[9]),(int16_t)((sample[10]<<8)|sample[11]),(int16_t)((sample[12]<<8)|sample[13]));fflush(stdout);vTaskDelay(pdMS_TO_TICKS(100));}
}

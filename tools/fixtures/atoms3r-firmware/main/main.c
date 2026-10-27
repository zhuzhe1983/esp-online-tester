// Internal ESP-IDF 5.5.1 protocol fixture, not a product firmware or Pages asset.
// Pin map: M5Stack AtomS3R C126 ST7735 batch official docs. Driver constants: M5GFX 65c816a3,
// M5Unified 368ad608. Tests actual guest SPI, I2C and GPIO through the WASM worker.
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "esp_psram.h"
#include "esp_heap_caps.h"
#include "esp_efuse.h"
#include "bmi270_config.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "driver/i2c.h"

static spi_device_handle_t lcd;
static void transfer(int dc, const void *bytes, size_t n) {
    gpio_set_level(42, dc);
    spi_transaction_t t = {.length=n*8, .tx_buffer=bytes};
    ESP_ERROR_CHECK(spi_device_polling_transmit(lcd,&t));
}
static void command(uint8_t command, const uint8_t *data, size_t n) {transfer(0,&command,1);if(n)transfer(1,data,n);}
static void reg_write(uint8_t reg,uint8_t value) {uint8_t bytes[]={reg,value};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x68,bytes,2,pdMS_TO_TICKS(100)));}
static void reg_read(uint8_t reg,uint8_t *bytes,size_t n) {ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_0,0x68,&reg,1,bytes,n,pdMS_TO_TICKS(100)));}
void app_main(void) {
    gpio_config_t outputs={.pin_bit_mask=(1ULL<<42)|(1ULL<<48),.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&outputs));
    gpio_set_level(48,0);vTaskDelay(pdMS_TO_TICKS(10));gpio_set_level(48,1);
    gpio_config_t button={.pin_bit_mask=1ULL<<41,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&button));
    spi_bus_config_t bus={.mosi_io_num=21,.miso_io_num=-1,.sclk_io_num=15,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=256};
    ESP_ERROR_CHECK(spi_bus_initialize(SPI3_HOST,&bus,SPI_DMA_DISABLED));
    spi_device_interface_config_t panel={.clock_speed_hz=1000000,.spics_io_num=14,.queue_size=1,.dummy_bits=1,.flags=SPI_DEVICE_3WIRE|SPI_DEVICE_HALFDUPLEX};ESP_ERROR_CHECK(spi_bus_add_device(SPI3_HOST,&panel,&lcd));
    command(0x04,NULL,0);gpio_set_level(42,1);uint8_t panel_id[4];spi_transaction_t read_id={.rxlength=32,.rx_buffer=panel_id};ESP_ERROR_CHECK(spi_device_polling_transmit(lcd,&read_id));printf("ATOMS3R_PANEL_ID %02x %02x %02x %02x\n",panel_id[0],panel_id[1],panel_id[2],panel_id[3]);
    command(0x11,NULL,0);command(0x29,NULL,0);command(0x3a,(uint8_t[]){0x55},1);command(0x36,(uint8_t[]){0xdc},1);
    command(0x2a,(uint8_t[]){0,2,0,129},4);command(0x2b,(uint8_t[]){0,3,0,130},4);command(0x2c,NULL,0);
    uint8_t line[256];for(int y=0;y<128;y++){for(int x=0;x<128;x++){uint16_t c=x<64?0xf800:0x07e0;line[x*2]=c>>8;line[x*2+1]=c;}for(int at=0;at<256;at+=64)transfer(1,line+at,64);}
    puts("ATOMS3R_LCD_READY 128x128 red-left green-right");
    i2c_config_t imu={.mode=I2C_MODE_MASTER,.sda_io_num=45,.scl_io_num=0,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&imu));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));uint8_t id;reg_read(0x00,&id,1);printf("ATOMS3R_IMU_ID %02x\n",id);
    reg_write(0x7e,0xb6);reg_write(0x7c,0);reg_write(0x59,0);
    for(int offset=0;offset<8192;offset+=64){reg_write(0x5b,(offset/2)&15);reg_write(0x5c,(offset/2)>>4);uint8_t data[65]={0x5e};memcpy(data+1,bmi270_config_file+offset,64);ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x68,data,65,pdMS_TO_TICKS(100)));}
    reg_write(0x59,1);reg_read(0x21,&id,1);printf("ATOMS3R_IMU_INIT %02x\n",id);reg_write(0x41,2);reg_write(0x43,0);reg_write(0x7d,6);
    const uint8_t light[][2]={{0,0x40},{8,1},{0x70,0},{14,255}};
    for(int i=0;i<4;i++)ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x30,light[i],2,pdMS_TO_TICKS(100)));
    puts("ATOMS3R_BACKLIGHT 255");
    printf("ATOMS3R_PACKAGE %lu\n",(unsigned long)esp_efuse_get_pkg_ver());
    uint8_t *ram=heap_caps_malloc(4096,MALLOC_CAP_SPIRAM);assert(ram);for(int i=0;i<4096;i++)ram[i]=(uint8_t)i;for(int i=0;i<4096;i++)assert(ram[i]==(uint8_t)i);free(ram);
    printf("ATOMS3R_PSRAM %u bytes read-write passed\n",(unsigned)esp_psram_get_size());
    int last=-1,presses=0;for(;;){int b=gpio_get_level(41);if(b!=last){printf("ATOMS3R_BUTTON %d\n",b);last=b;if(!b){uint8_t levels[]={0,128,255};uint8_t light[]={14,levels[presses++%3]};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x30,light,2,pdMS_TO_TICKS(100)));printf("ATOMS3R_BACKLIGHT %u\n",light[1]);}}
        uint8_t sample[12];reg_read(0x0c,sample,12);printf("ATOMS3R_ACCEL %d %d %d\n",(int16_t)((sample[1]<<8)|sample[0]),(int16_t)((sample[3]<<8)|sample[2]),(int16_t)((sample[5]<<8)|sample[4]));printf("ATOMS3R_GYRO %d %d %d\n",(int16_t)((sample[7]<<8)|sample[6]),(int16_t)((sample[9]<<8)|sample[8]),(int16_t)((sample[11]<<8)|sample[10]));fflush(stdout);vTaskDelay(pdMS_TO_TICKS(100));}
}

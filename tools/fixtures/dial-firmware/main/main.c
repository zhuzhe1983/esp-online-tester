// Internal ESP-IDF 5.5.1 fixture, not official firmware or a Pages asset.
// M5Dial K130: manufacturer schematic / M5GFX 65c816a3 / M5Unified 368ad608.
#include <stdio.h>
#include <stdint.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "driver/i2c.h"
static spi_device_handle_t lcd;
static volatile int encoder_count,encoder_state=3;
static void encoder_edge(void *arg) {
    (void)arg;int next=gpio_get_level(41)|(gpio_get_level(40)<<1);
    // Canonical Gray code; A falls first => -4 raw edge counts / detent.
    const int8_t changes[]={0,-1,1,0,1,0,0,-1,-1,0,0,1,0,1,-1,0};
    encoder_count+=changes[(encoder_state<<2)|next];encoder_state=next;
}
static void transfer(int dc,const void *bytes,size_t n) {gpio_set_level(4,dc);spi_transaction_t t={.length=n*8,.tx_buffer=bytes};ESP_ERROR_CHECK(spi_device_polling_transmit(lcd,&t));}
static void command(uint8_t c,const uint8_t *data,size_t n) {transfer(0,&c,1);if(n)transfer(1,data,n);}
void app_main(void) {
    gpio_config_t out={.pin_bit_mask=(1ULL<<4)|(1ULL<<8)|(1ULL<<9)|(1ULL<<46),.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&out));gpio_set_level(46,1);gpio_set_level(8,0);vTaskDelay(pdMS_TO_TICKS(10));gpio_set_level(8,1);gpio_set_level(9,1);
    gpio_config_t buttons={.pin_bit_mask=(1ULL<<42)|(1ULL<<14),.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&buttons));
    gpio_config_t enc={.pin_bit_mask=(1ULL<<41)|(1ULL<<40),.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE,.intr_type=GPIO_INTR_ANYEDGE};ESP_ERROR_CHECK(gpio_config(&enc));ESP_ERROR_CHECK(gpio_install_isr_service(0));ESP_ERROR_CHECK(gpio_isr_handler_add(41,encoder_edge,NULL));ESP_ERROR_CHECK(gpio_isr_handler_add(40,encoder_edge,NULL));
    spi_bus_config_t bus={.mosi_io_num=5,.miso_io_num=-1,.sclk_io_num=6,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=64};ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST,&bus,SPI_DMA_DISABLED));
    spi_device_interface_config_t panel={.clock_speed_hz=1000000,.spics_io_num=7,.queue_size=1};ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&panel,&lcd));
    command(0x11,NULL,0);command(0x29,NULL,0);command(0x3a,(uint8_t[]){0x55},1);command(0x36,(uint8_t[]){0},1);command(0x2a,(uint8_t[]){0,0,0,239},4);command(0x2b,(uint8_t[]){0,0,0,239},4);command(0x2c,NULL,0);
    uint8_t pixels[32];for(int y=0;y<240;y++){for(int x=0;x<240;x+=16){uint16_t c=x<128?0xf800:0x07e0;for(int i=0;i<16;i++){pixels[2*i]=c>>8;pixels[2*i+1]=c;}transfer(1,pixels,32);}}
    puts("DIAL_LCD_READY 240x240 red128 green112");
    i2c_config_t touch={.mode=I2C_MODE_MASTER,.sda_io_num=11,.scl_io_num=12,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_1,&touch));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_1,I2C_MODE_MASTER,0,0,0));
    uint8_t reg=0xa3,ids[6];ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_1,0x38,&reg,1,ids,6,pdMS_TO_TICKS(100)));printf("DIAL_TOUCH_VENDOR %02x\n",ids[5]);
    int last_button=-1,last_encoder=999,last_touch=-1,last_x=-1,last_y=-1;
    for(;;){int b=gpio_get_level(42),count=encoder_count;if(b!=last_button){printf("DIAL_BUTTON %d\n",b);last_button=b;}if(count!=last_encoder){printf("DIAL_ENCODER %d\n",count);last_encoder=count;}
        uint8_t data[5];reg=2;ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_1,0x38,&reg,1,data,5,pdMS_TO_TICKS(100)));int down=data[0]&15,x=((data[1]&15)<<8)|data[2],y=((data[3]&15)<<8)|data[4];if(down!=last_touch||x!=last_x||y!=last_y){printf("DIAL_TOUCH %d %d %d IRQ=%d\n",down,x,y,gpio_get_level(14));last_touch=down;last_x=x;last_y=y;}fflush(stdout);vTaskDelay(pdMS_TO_TICKS(20));}
}

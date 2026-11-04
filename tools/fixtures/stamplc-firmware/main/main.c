// Internal IDF 5.5.1 test; never manufacturer/factory firmware or a public asset.
// K141 CPU/BASE v1.0 schematics + M5StamPLC-UserDemo 7fc748c3 hal_stamplc.
#include <stdio.h>
#include <stdint.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/i2c.h"
#include "driver/spi_master.h"
#include "driver/rmt_tx.h"
static spi_device_handle_t lcd,wrong_lcd;
static rmt_channel_handle_t stamp_led;
static rmt_encoder_handle_t stamp_encoder;
static esp_err_t read_at(int bus,uint8_t address,uint8_t reg,uint8_t *value) {return i2c_master_write_read_device(bus,address,&reg,1,value,1,pdMS_TO_TICKS(100));}
static uint8_t rd(uint8_t address,uint8_t reg) {uint8_t value;ESP_ERROR_CHECK(read_at(0,address,reg,&value));return value;}
static void wr(uint8_t address,uint8_t reg,uint8_t value) {uint8_t data[]={reg,value};ESP_ERROR_CHECK(i2c_master_write_to_device(0,address,data,sizeof(data),pdMS_TO_TICKS(100)));}
static void bus_init(int bus,int sda,int scl) {i2c_config_t c={.mode=I2C_MODE_MASTER,.sda_io_num=sda,.scl_io_num=scl,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};ESP_ERROR_CHECK(i2c_param_config(bus,&c));ESP_ERROR_CHECK(i2c_driver_install(bus,I2C_MODE_MASTER,0,0,0));}
static void stage(const char *name) {printf("STAMPLC_STAGE %s\n",name);fflush(stdout);vTaskDelay(pdMS_TO_TICKS(1000));}
static void tx(int dc,const void *data,size_t n) {gpio_set_level(6,dc);spi_transaction_t t={.length=n*8,.tx_buffer=data};ESP_ERROR_CHECK(spi_device_polling_transmit(lcd,&t));}
static void cmd(uint8_t c,const uint8_t *data,size_t n) {tx(0,&c,1);if(n)tx(1,data,n);}
static void paint(int pressed) {
    cmd(0x2a,(uint8_t[]){0,40,1,23},4);cmd(0x2b,(uint8_t[]){0,53,0,187},4);cmd(0x2c,NULL,0);
    uint8_t data[32];for(int y=0;y<135;y++)for(int x=0;x<240;x+=16){for(int i=0;i<16;i++){uint16_t c=pressed?((pressed&4?0xf800:0)|(pressed&2?0x07e0:0)|(pressed&1?0x001f:0)):(y<67?(x+i<120?0xf800:0x07e0):(x+i<120?0x001f:0xffff));data[2*i]=c>>8;data[2*i+1]=c;}tx(1,data,sizeof(data));}
}
static void rgb_stamp(uint8_t r,uint8_t g,uint8_t b) {
    uint32_t grb=((uint32_t)g<<16)|((uint32_t)r<<8)|b;rmt_symbol_word_t symbols[25];
    for(int i=0;i<24;i++){int one=(grb&(1U<<(23-i)))!=0;symbols[i]=(rmt_symbol_word_t){.level0=1,.duration0=one?8:4,.level1=0,.duration1=one?4:8};}symbols[24]=(rmt_symbol_word_t){.level0=0,.duration0=3000,.level1=0,.duration1=3000};
    rmt_transmit_config_t config={0};ESP_ERROR_CHECK(rmt_transmit(stamp_led,stamp_encoder,symbols,sizeof(symbols),&config));ESP_ERROR_CHECK(rmt_tx_wait_all_done(stamp_led,1000));
}
void app_main(void) {
    gpio_config_t out={.pin_bit_mask=(1ULL<<3)|(1ULL<<6)|(1ULL<<38),.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&out));gpio_set_level(3,0);gpio_set_level(38,0);
    gpio_config_t irq={.pin_bit_mask=1ULL<<14,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&irq));
    bus_init(0,13,15);bus_init(1,2,1);uint8_t value;
    printf("STAMPLC_RESET_HELD %s %s\n",read_at(0,0x43,1,&value)==ESP_OK?"ACK":"NACK",read_at(0,0x59,0x10,&value)==ESP_OK?"ACK":"NACK");
    gpio_set_level(3,1);vTaskDelay(pdMS_TO_TICKS(10));printf("STAMPLC_IDS %02x %02x\n",rd(0x43,1),rd(0x59,0x10));
    printf("STAMPLC_WRONG_ADDRESS %s\n",read_at(0,0x44,1,&value)==ESP_OK?"ACK":"NACK");printf("STAMPLC_WRONG_PINS %s\n",read_at(1,0x43,1,&value)==ESP_OK?"ACK":"NACK");ESP_ERROR_CHECK(i2c_driver_delete(1));
    // PI4IO output bits4..7; internal pull-ups for P0..2; active-low RGB off via high-Z.
    wr(0x43,3,0xf0);wr(0x43,5,0);wr(0x43,7,0xff);wr(0x43,11,7);wr(0x43,13,7);wr(0x43,9,7);wr(0x43,17,0xf8);rd(0x43,19);
    printf("STAMPLC_AW_DEFAULT %02x %02x INPUT %02x\n",rd(0x59,2),rd(0x59,3),rd(0x59,0));
    wr(0x59,4,0xf0);wr(0x59,5,0xff);wr(0x59,2,0);wr(0x59,0x11,0x10);wr(0x59,6,0xff);wr(0x59,7,0xff);
    spi_bus_config_t bus={.mosi_io_num=8,.miso_io_num=9,.sclk_io_num=7,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=64};ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST,&bus,SPI_DMA_DISABLED));
    spi_device_interface_config_t config={.clock_speed_hz=10000000,.spics_io_num=12,.queue_size=1};ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&config,&lcd));config.spics_io_num=10;ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&config,&wrong_lcd));
    gpio_set_level(6,0);uint8_t command=4,id[3];spi_transaction_t t={.length=8,.tx_buffer=&command};ESP_ERROR_CHECK(spi_device_polling_transmit(wrong_lcd,&t));gpio_set_level(6,1);t=(spi_transaction_t){.length=24,.rx_buffer=id};ESP_ERROR_CHECK(spi_device_polling_transmit(wrong_lcd,&t));printf("STAMPLC_WRONG_CS %02x%02x%02x\n",id[0],id[1],id[2]);
    cmd(0x11,NULL,0);cmd(0x29,NULL,0);cmd(0x3a,(uint8_t[]){0x55},1);cmd(0x36,(uint8_t[]){0x64},1);paint(0);stage("BACKLIGHT_OFF");wr(0x43,7,0x7f);stage("FOUR_CORNERS");
    wr(0x59,2,8);printf("STAMPLC_RELAY4 %02x\n",rd(0x59,0)&15);wr(0x59,2,0);
    rmt_tx_channel_config_t led={.gpio_num=21,.clk_src=RMT_CLK_SRC_DEFAULT,.resolution_hz=10000000,.mem_block_symbols=48,.trans_queue_depth=1};rmt_copy_encoder_config_t copy={};ESP_ERROR_CHECK(rmt_new_tx_channel(&led,&stamp_led));ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&stamp_encoder));ESP_ERROR_CHECK(rmt_enable(stamp_led));
    rgb_stamp(32,0,16);stage("STAMP_GATE_LOW");gpio_set_level(38,1);stage("STAMP_GATE_HIGH_NO_FRAME");rgb_stamp(32,0,16);puts("STAMPLC_READY");fflush(stdout);
    int last=-1;for(;;){int pressed=(~rd(0x43,15))&7;if(pressed!=last){int irq_before=gpio_get_level(14);int pending=rd(0x43,19);int irq_after=gpio_get_level(14);int relays=((pressed&4)>>2)|(pressed&2)|((pressed&1)<<2);wr(0x59,2,relays);wr(0x43,7,0x7f&~((pressed&4?0x40:0)|(pressed&2?0x20:0)|(pressed&1?0x10:0)));paint(pressed);printf("STAMPLC_BUTTONS %d IRQ %d %02x %d RELAYS %02x\n",pressed,irq_before,pending,irq_after,rd(0x59,0)&15);last=pressed;fflush(stdout);}vTaskDelay(pdMS_TO_TICKS(20));}
}

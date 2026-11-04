// Internal IDF 5.5.1 protocol fixture, never factory firmware or a public asset.
// Official S015 schematic; M5PM1 315d2899 M5PM1.h/.cpp and S3Bat RGB tutorial.
#include <stdio.h>
#include <stdint.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/rmt_tx.h"
#include "esp_psram.h"

static esp_err_t read_at(int bus,uint8_t address,uint8_t reg,uint8_t *value) {
    return i2c_master_write_read_device(bus,address,&reg,1,value,1,pdMS_TO_TICKS(100));
}
static uint8_t rd(uint8_t reg) {uint8_t value;ESP_ERROR_CHECK(read_at(0,0x6e,reg,&value));return value;}
static void wr(uint8_t reg,uint8_t value) {uint8_t data[]={reg,value};ESP_ERROR_CHECK(i2c_master_write_to_device(0,0x6e,data,sizeof(data),pdMS_TO_TICKS(100)));}
static void color(uint16_t value) {wr(0x60,value&255);wr(0x61,value>>8);}
static void stage(const char *name) {printf("S3BAT_STAGE %s\n",name);fflush(stdout);vTaskDelay(pdMS_TO_TICKS(1000));}
static void bus_init(int bus,int sda,int scl) {i2c_config_t c={.mode=I2C_MODE_MASTER,.sda_io_num=sda,.scl_io_num=scl,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};ESP_ERROR_CHECK(i2c_param_config(bus,&c));ESP_ERROR_CHECK(i2c_driver_install(bus,I2C_MODE_MASTER,0,0,0));}
static void wrong_rmt(void) {
    rmt_channel_handle_t channel;rmt_encoder_handle_t encoder;
    rmt_tx_channel_config_t c={.gpio_num=21,.clk_src=RMT_CLK_SRC_DEFAULT,.resolution_hz=10000000,.mem_block_symbols=48,.trans_queue_depth=1};
    rmt_copy_encoder_config_t copy={};ESP_ERROR_CHECK(rmt_new_tx_channel(&c,&channel));ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&encoder));ESP_ERROR_CHECK(rmt_enable(channel));
    rmt_symbol_word_t bits[25];for(int i=0;i<24;i++)bits[i]=(rmt_symbol_word_t){.level0=1,.duration0=8,.level1=0,.duration1=4};bits[24]=(rmt_symbol_word_t){.level0=0,.duration0=3000,.level1=0,.duration1=3000};
    rmt_transmit_config_t t={0};ESP_ERROR_CHECK(rmt_transmit(channel,encoder,bits,sizeof(bits),&t));ESP_ERROR_CHECK(rmt_tx_wait_all_done(channel,1000));ESP_ERROR_CHECK(rmt_disable(channel));ESP_ERROR_CHECK(rmt_del_channel(channel));ESP_ERROR_CHECK(rmt_del_encoder(encoder));
}
void app_main(void) {
    printf("S3BAT_PSRAM %u\n",(unsigned)esp_psram_get_size());
    bus_init(0,48,47);bus_init(1,3,4);uint8_t other;
    printf("S3BAT_IDENT %02x %02x %02x %02x POWER %02x\n",rd(0),rd(1),rd(2),rd(3),rd(6));
    printf("S3BAT_WRONG_ADDRESS %s\n",read_at(0,0x6f,0,&other)==ESP_OK?"ACK":"NACK");
    printf("S3BAT_WRONG_PINS %s\n",read_at(1,0x6e,0,&other)==ESP_OK?"ACK":"NACK");
    ESP_ERROR_CHECK(i2c_driver_delete(1));
    // M5PM1 GPIO0 other function, push-pull, output. Power remains off.
    wr(0x16,3);wr(0x13,rd(0x13)&~1);wr(0x10,1);color(0xf800);wr(0x50,0x41);stage("POWER_OFF");
    wr(6,0x17);stage("POWER_ON_NO_REFRESH");wr(0x50,0x41);stage("RED");
    color(0x07e0);wrong_rmt();stage("BUFFER_GREEN_RMT21_IGNORED");
    wr(6,7);stage("POWER_CUT");wr(6,0x17);stage("RESTORED_NO_REFRESH");wr(0x50,0x41);stage("GREEN_REFRESH");
    // Disable the two autonomous PM1 actions before exposing physical-button polling.
    wr(0x49,rd(0x49)|1);wr(0x4a,1);color(0xf800);wr(0x50,0x41);puts("S3BAT_READY");fflush(stdout);
    int last=-1,step=0;const uint16_t colors[]={0xf800,0x07e0,0x001f,0};
    for(;;){uint8_t v=rd(0x48);int pressed=v&1;if(pressed!=last){printf("S3BAT_BUTTON %d FIRST %02x SECOND %02x\n",pressed,v,rd(0x48));if(pressed){step=(step+1)%4;color(colors[step]);wr(0x50,0x41);printf("S3BAT_OUTPUT %d\n",step);}last=pressed;fflush(stdout);}vTaskDelay(pdMS_TO_TICKS(20));}
}

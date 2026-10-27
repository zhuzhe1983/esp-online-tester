// Internal IDF 5.5.1 C124 fixture; never part of the public firmware catalog.
// Official M5AtomS3 440c112f LedDisplay.h: one NeoPixel on G35. Button G41.
#include <stdio.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/rmt_tx.h"
#include "esp_cpu.h"
#include "esp_rom_sys.h"
#include "soc/gpio_reg.h"
#include "soc/soc.h"

static rmt_channel_handle_t channel;
static rmt_encoder_handle_t encoder;
static void rmt_rgb(uint8_t r,uint8_t g,uint8_t b) {
    rmt_symbol_word_t symbols[25];uint32_t grb=((uint32_t)g<<16)|((uint32_t)r<<8)|b;
    for(int i=0;i<24;i++){int one=(grb&(1U<<(23-i)))!=0;symbols[i]=(rmt_symbol_word_t){.level0=1,.duration0=one?8:4,.level1=0,.duration1=one?4:8};}
    symbols[24]=(rmt_symbol_word_t){.level0=0,.duration0=3000,.level1=0,.duration1=3000};
    rmt_transmit_config_t tx={.loop_count=0};ESP_ERROR_CHECK(rmt_transmit(channel,encoder,symbols,sizeof(symbols),&tx));ESP_ERROR_CHECK(rmt_tx_wait_all_done(channel,1000));
    printf("ATOMS3_LITE_RMT %u %u %u\n",r,g,b);
}
static inline void wait_cycles(uint32_t start,uint32_t ticks) {while((uint32_t)(esp_cpu_get_cycle_count()-start)<ticks){}}
static void gpio_rgb(uint8_t r,uint8_t g,uint8_t b) {
    uint32_t grb=((uint32_t)g<<16)|((uint32_t)r<<8)|b;
    // ESP-IDF v5.5.1 gpio_reg.h: OUT1 W1TS/W1TC bit3 is GPIO35; v4.4 is the same.
    // Direct writes avoid driver-call latency overwhelming sub-microsecond pulses.
    for(int i=0;i<24;i++){uint32_t t=esp_cpu_get_cycle_count();REG_WRITE(GPIO_OUT1_W1TS_REG,1U<<3);wait_cycles(t,(grb&(1U<<(23-i)))?192:96);REG_WRITE(GPIO_OUT1_W1TC_REG,1U<<3);wait_cycles(t,300);}
    esp_rom_delay_us(300);printf("ATOMS3_LITE_GPIO %u %u %u\n",r,g,b);
}
void app_main(void) {
    gpio_config_t key={.pin_bit_mask=1ULL<<41,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&key));
    gpio_config_t led={.pin_bit_mask=1ULL<<35,.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&led));
    // Exercise the CPU's real GPIO waveform before attaching the RMT route.
    gpio_rgb(24,48,96);
    rmt_tx_channel_config_t config={.gpio_num=35,.clk_src=RMT_CLK_SRC_DEFAULT,.resolution_hz=10000000,.mem_block_symbols=48,.trans_queue_depth=1};
    ESP_ERROR_CHECK(rmt_new_tx_channel(&config,&channel));rmt_copy_encoder_config_t copy={0};ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&encoder));ESP_ERROR_CHECK(rmt_enable(channel));
    // Keep each startup color observable by a worker/UI poll.
    vTaskDelay(pdMS_TO_TICKS(1000));rmt_rgb(255,0,0);
    int last=-1,step=0;
    for(;;){int level=gpio_get_level(41);if(level!=last){printf("ATOMS3_LITE_BUTTON %d\n",level);if(!level){step=(step+1)%4;if(step==1)rmt_rgb(0,255,0);else if(step==2)rmt_rgb(0,0,255);else if(step==3)rmt_rgb(0,0,0);else rmt_rgb(255,0,0);}last=level;fflush(stdout);}vTaskDelay(pdMS_TO_TICKS(20));}
}

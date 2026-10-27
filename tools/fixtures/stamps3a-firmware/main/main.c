// Internal IDF 5.5.1 S007-V033 fixture; never part of the public firmware catalog.
// Stamp-S3A schematic v0.3.3 + official Arduino quickstart: WS2812B G21, button G0, LED_EN G38.
#include <stdio.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/rmt_tx.h"

static rmt_channel_handle_t channel;
static rmt_encoder_handle_t encoder;
static void rmt_rgb(uint8_t r,uint8_t g,uint8_t b) {
    rmt_symbol_word_t symbols[25];uint32_t grb=((uint32_t)g<<16)|((uint32_t)r<<8)|b;
    for(int i=0;i<24;i++){int one=(grb&(1U<<(23-i)))!=0;symbols[i]=(rmt_symbol_word_t){.level0=1,.duration0=one?8:4,.level1=0,.duration1=one?4:8};}
    symbols[24]=(rmt_symbol_word_t){.level0=0,.duration0=3000,.level1=0,.duration1=3000};
    rmt_transmit_config_t tx={.loop_count=0};ESP_ERROR_CHECK(rmt_transmit(channel,encoder,symbols,sizeof(symbols),&tx));ESP_ERROR_CHECK(rmt_tx_wait_all_done(channel,1000));
    printf("STAMPS3A_RMT %u %u %u\n",r,g,b);
}
void app_main(void) {
    gpio_config_t key={.pin_bit_mask=1ULL<<0,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&key));
    gpio_config_t led={.pin_bit_mask=1ULL<<21,.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&led));
    rmt_tx_channel_config_t config={.gpio_num=21,.clk_src=RMT_CLK_SRC_DEFAULT,.resolution_hz=10000000,.mem_block_symbols=48,.trans_queue_depth=1};
    ESP_ERROR_CHECK(rmt_new_tx_channel(&config,&channel));rmt_copy_encoder_config_t copy={0};ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&encoder));ESP_ERROR_CHECK(rmt_enable(channel));
    gpio_config_t supply={.pin_bit_mask=1ULL<<38,.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&supply));
    ESP_ERROR_CHECK(gpio_set_level(38,0));rmt_rgb(255,255,255);printf("STAMPS3A_SUPPLY 0 ignored-white\n");vTaskDelay(pdMS_TO_TICKS(1000));
    ESP_ERROR_CHECK(gpio_set_level(38,1));printf("STAMPS3A_SUPPLY 1 no-restored-frame\n");
    // Keep each startup color observable by a worker/UI poll.
    vTaskDelay(pdMS_TO_TICKS(1000));rmt_rgb(255,0,0);
    int last=-1,step=0;
    for(;;){int level=gpio_get_level(0);if(level!=last){printf("STAMPS3A_BUTTON %d\n",level);if(!level){step=(step+1)%5;if(step==1)rmt_rgb(0,255,0);else if(step==2){gpio_set_level(38,0);rmt_rgb(0,0,255);printf("STAMPS3A_SUPPLY 0 ignored-blue\n");}else if(step==3){gpio_set_level(38,1);printf("STAMPS3A_SUPPLY 1 no-restored-frame\n");vTaskDelay(pdMS_TO_TICKS(500));rmt_rgb(0,0,255);}else if(step==4){gpio_set_direction(38,GPIO_MODE_INPUT);printf("STAMPS3A_SUPPLY input\n");}else{gpio_set_direction(38,GPIO_MODE_OUTPUT);gpio_set_level(38,1);rmt_rgb(255,0,0);}}last=level;fflush(stdout);}vTaskDelay(pdMS_TO_TICKS(20));}
}

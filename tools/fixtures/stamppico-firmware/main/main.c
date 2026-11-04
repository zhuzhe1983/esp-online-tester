// Independent IDF5.5.1 fixture: G39 external pull-up, G27 GRB; not public/factory firmware.
#include <stdio.h>
#define BUTTON_PIN 39
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/rmt_tx.h"

static rmt_channel_handle_t channel;
static rmt_encoder_handle_t encoder;
static void send_rgb(rmt_channel_handle_t target,uint8_t r,uint8_t g,uint8_t b) {
    rmt_symbol_word_t symbols[25];uint32_t grb=((uint32_t)g<<16)|((uint32_t)r<<8)|b;
    for(int i=0;i<24;i++){int one=(grb&(1U<<(23-i)))!=0;symbols[i]=(rmt_symbol_word_t){.level0=1,.duration0=one?8:4,.level1=0,.duration1=one?4:8};}
    symbols[24]=(rmt_symbol_word_t){.level0=0,.duration0=3000,.level1=0,.duration1=3000};
    rmt_transmit_config_t tx={.loop_count=0};ESP_ERROR_CHECK(rmt_transmit(target,encoder,symbols,sizeof(symbols),&tx));ESP_ERROR_CHECK(rmt_tx_wait_all_done(target,1000));
}
static void rgb(uint8_t r,uint8_t g,uint8_t b) {send_rgb(channel,r,g,b);printf("STAMPPICO_RMT %u %u %u\n",r,g,b);}
void app_main(void) {
    printf("STAMPPICO_PINS %u 27\n",BUTTON_PIN);
    gpio_config_t key={.pin_bit_mask=1ULL<<BUTTON_PIN,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_DISABLE};ESP_ERROR_CHECK(gpio_config(&key));
    rmt_copy_encoder_config_t copy={0};ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&encoder));
    rmt_channel_handle_t wrong;
    rmt_tx_channel_config_t config={.gpio_num=4,.clk_src=RMT_CLK_SRC_DEFAULT,.resolution_hz=10000000,.mem_block_symbols=64,.trans_queue_depth=1};
    ESP_ERROR_CHECK(rmt_new_tx_channel(&config,&wrong));ESP_ERROR_CHECK(rmt_enable(wrong));send_rgb(wrong,255,255,255);printf("STAMPPICO_WRONG_PIN 4 white\n");vTaskDelay(pdMS_TO_TICKS(1000));
    ESP_ERROR_CHECK(rmt_disable(wrong));ESP_ERROR_CHECK(rmt_del_channel(wrong));config.gpio_num=27;
    ESP_ERROR_CHECK(rmt_new_tx_channel(&config,&channel));ESP_ERROR_CHECK(rmt_enable(channel));rgb(255,0,0);
    int last=-1,step=0;
    for(;;){int level=gpio_get_level(BUTTON_PIN);if(level!=last){printf("STAMPPICO_BUTTON %d\n",level);if(!level){step=(step+1)%4;if(step==1)rgb(0,255,0);else if(step==2)rgb(0,0,255);else if(step==3)rgb(0,0,0);else rgb(255,0,0);}last=level;fflush(stdout);}vTaskDelay(pdMS_TO_TICKS(20));}
}

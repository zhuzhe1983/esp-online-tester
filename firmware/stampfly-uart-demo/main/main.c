// SPDX-License-Identifier: MIT
// Independently authored public UART demonstration, not M5Stack factory firmware.
// Routes: StampFly schematic v1.0 / M5StampFly 2b96752c; IDF5.5.1 drivers.
#include <stdio.h>
#include <math.h>
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/ledc.h"
#include "driver/rmt_tx.h"
#include "driver/uart.h"
#include "esp_timer.h"
#include "control.h"
#include "sensors.h"
static sf_control control;
static sf_sample sample;
static rmt_channel_handle_t led;
static rmt_encoder_handle_t encoder;
static void color(sf_mode mode){
    const uint8_t colors[][3]={{24,0,0},{0,0,32},{0,32,0},{24,16,0}};
    const uint8_t *p=colors[mode];uint32_t grb=(p[1]<<16)|(p[0]<<8)|p[2];rmt_symbol_word_t symbols[25];
    for(int bit=0;bit<24;bit++){bool one=grb&(1U<<(23-bit));symbols[bit]=(rmt_symbol_word_t){.level0=1,.duration0=one?8:4,.level1=0,.duration1=one?4:8};}
    symbols[24]=(rmt_symbol_word_t){.level0=0,.duration0=3000,.level1=0,.duration1=3000};rmt_transmit_config_t tx={0};
    ESP_ERROR_CHECK(rmt_transmit(led,encoder,symbols,sizeof symbols,&tx));ESP_ERROR_CHECK(rmt_tx_wait_all_done(led,100));
}
static void motors(void){for(int i=0;i<4;i++){uint32_t duty=(uint32_t)fminf(255,lroundf(control.duty[i]*256));ESP_ERROR_CHECK(ledc_set_duty(LEDC_LOW_SPEED_MODE,i,duty));ESP_ERROR_CHECK(ledc_update_duty(LEDC_LOW_SPEED_MODE,i));}}
void app_main(void){
    const int pins[]={5,42,10,41};ledc_timer_config_t timer={.speed_mode=LEDC_LOW_SPEED_MODE,.duty_resolution=LEDC_TIMER_8_BIT,.timer_num=LEDC_TIMER_0,.freq_hz=150000,.clk_cfg=LEDC_AUTO_CLK};ESP_ERROR_CHECK(ledc_timer_config(&timer));
    for(int i=0;i<4;i++){ledc_channel_config_t channel={.gpio_num=pins[i],.speed_mode=LEDC_LOW_SPEED_MODE,.channel=i,.timer_sel=LEDC_TIMER_0,.duty=0};ESP_ERROR_CHECK(ledc_channel_config(&channel));}
    gpio_config_t gate={.pin_bit_mask=1ULL<<38,.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&gate));ESP_ERROR_CHECK(gpio_set_level(38,1));
    rmt_copy_encoder_config_t copy={};ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&encoder));rmt_tx_channel_config_t rgb={.gpio_num=21,.clk_src=RMT_CLK_SRC_DEFAULT,.resolution_hz=10000000,.mem_block_symbols=48,.trans_queue_depth=1};ESP_ERROR_CHECK(rmt_new_tx_channel(&rgb,&led));ESP_ERROR_CHECK(rmt_enable(led));color(SF_STOP);
    // UART0 transport is the simulator's logical UART ingress; no physical serial wiring is claimed.
    ESP_ERROR_CHECK(uart_driver_install(UART_NUM_0,512,0,0,NULL,0));
    setvbuf(stdout,NULL,_IONBF,0);sf_sensors_init();printf("SF1 READY stampfly-uart-demo %s\n",SF_VERSION);
    char line[SF_LINE_CAPACITY];size_t length=0;bool overflow=false;TickType_t tick=xTaskGetTickCount();int64_t last=esp_timer_get_time(),last_state=0,last_sensor=0;sf_mode shown=SF_STOP;
    for(;;){
        uint8_t bytes[64];int count=uart_read_bytes(UART_NUM_0,bytes,sizeof bytes,0);
        for(int i=0;i<count;i++){
            if(bytes[i]=='\n'){
                if(overflow)puts("SF1 ERR LINE_TOO_LONG");else {line[length]=0;const char *message;bool accepted=sf_command(&control,&sample,line,&message);motors();printf("SF1 %s %s\n",accepted?"ACK":"ERR",message);}
                length=0;overflow=false;
            }else if(!overflow){if(length+1<sizeof line&&bytes[i]!=0)line[length++]=bytes[i];else overflow=true;}
        }
        int64_t now=esp_timer_get_time();float dt=(now-last)*1e-6f;sf_sensors_update(&sample,dt,now,control.mode==SF_STOP);sf_step(&control,&sample,dt);last=now;motors();
        if(shown!=control.mode){shown=control.mode;color(shown);}
        if(now-last_state>=200000){last_state=now;printf("SF1 STATE %s %llu %.3f %.2f %.2f %.2f %.4f %.4f %.4f %.4f %d %d\n",sf_mode_name(control.mode),(unsigned long long)(now/1000),sample.height,sample.roll,sample.pitch,sample.r,control.duty[0],control.duty[1],control.duty[2],control.duty[3],sample.imu_valid,sample.tof_valid);}
        if(now-last_sensor>=1000000){last_sensor=now;sf_sensors_report();}
        vTaskDelayUntil(&tick,pdMS_TO_TICKS(5));
    }
}

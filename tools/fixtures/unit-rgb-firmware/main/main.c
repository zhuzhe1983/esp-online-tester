// Internal ESP-IDF 5.5.1 fixture. Outputs travel through RMT TX, GPIO matrix,
// StopWatch Port A G10 (yellow), and U003 J2.2. Never a public product firmware.
#include <stdio.h>
#include <stdint.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/uart.h"
#include "driver/rmt_tx.h"
#include "esp_system.h"
static rmt_channel_handle_t channels[2];
static rmt_encoder_handle_t encoder;
static void power_reg(uint8_t reg,uint8_t value) {
    uint8_t bytes[]={reg,value};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x6e,bytes,2,pdMS_TO_TICKS(100)));
}
static void show(int channel,int offset) {
    const uint8_t colors[3][3]={{255,0,0},{0,128,0},{0,0,64}};
    rmt_symbol_word_t symbols[73]={0};int at=0;
    for(int i=0;i<3;i++)for(int c=0;c<3;c++) {
        uint8_t byte=colors[(i+offset)%3][c==0?1:c==1?0:2];
        for(int b=7;b>=0;b--)symbols[at++]=(rmt_symbol_word_t){.level0=1,.duration0=(byte&(1<<b))?6:3,.level1=0,.duration1=(byte&(1<<b))?6:9};
    }
    symbols[72]=(rmt_symbol_word_t){.level0=0,.duration0=800,.level1=0,.duration1=800}; // 160 us reset at 10 MHz
    rmt_transmit_config_t cfg={.loop_count=0};ESP_ERROR_CHECK(rmt_transmit(channels[channel],encoder,symbols,sizeof(symbols),&cfg));ESP_ERROR_CHECK(rmt_tx_wait_all_done(channels[channel],1000));
    printf("U003_TX pin=%d offset=%d bytes=9\n",channel?11:10,offset);fflush(stdout);
}
void app_main(void) {
    i2c_config_t bus={.mode=I2C_MODE_MASTER,.sda_io_num=47,.scl_io_num=48,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    ESP_ERROR_CHECK(uart_driver_install(UART_NUM_0,256,0,0,NULL,0));
    rmt_copy_encoder_config_t copy={};ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&encoder));
    for(int i=0;i<2;i++){rmt_tx_channel_config_t cfg={.clk_src=RMT_CLK_SRC_DEFAULT,.gpio_num=10+i,.resolution_hz=10000000,.mem_block_symbols=64,.trans_queue_depth=4};ESP_ERROR_CHECK(rmt_new_tx_channel(&cfg,&channels[i]));ESP_ERROR_CHECK(rmt_enable(channels[i]));}
    power_reg(6,0x17);puts("U003_READY POWER_OFF");show(0,2);fflush(stdout);
    vTaskDelay(pdMS_TO_TICKS(300));power_reg(6,0x1f);show(0,0);puts("U003_BOOT_READY");fflush(stdout);
    for(;;) {uint8_t key;if(uart_read_bytes(UART_NUM_0,&key,1,pdMS_TO_TICKS(10))!=1)continue;
        switch(key){case 'a':show(0,0);break;case 'b':show(0,1);break;case 'w':show(1,2);break;
        case 'i':power_reg(6,0x1f);puts("U003_POWER_ON");break;case 'o':power_reg(6,0x17);puts("U003_POWER_OFF");break;
        case 'h':power_reg(7,0x40);puts("U003_HOLD");break;case 'n':power_reg(7,0);puts("U003_NO_HOLD");break;case 's':power_reg(0x0c,0xa1);puts("U003_SHUTDOWN");break;
        case 'r':puts("U003_RESTART");fflush(stdout);esp_restart();break;default:break;}fflush(stdout);
    }
}

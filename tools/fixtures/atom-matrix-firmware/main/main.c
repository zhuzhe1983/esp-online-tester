// Internal ESP-IDF 5.5.1 fixture, not a manufacturer image. C008-B only.
// Official Atom-Matrix docs / M5Atom ebbf7363: key39, RGB27, MPU6886 SDA25/SCL21.
#include <stdio.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/rmt_tx.h"
#include "driver/i2c.h"
static rmt_channel_handle_t channel;
static rmt_encoder_handle_t encoder;
static void pixels(int pressed) {
    rmt_symbol_word_t symbols[601];
    for(int led=0;led<25;led++) {
        unsigned r=led==0?(pressed?0:255):0,g=led==0&&pressed?255:0,b=led==24?255:0;
        uint32_t grb=(g<<16)|(r<<8)|b;
        for(int i=0;i<24;i++){int one=(grb&(1U<<(23-i)))!=0;symbols[led*24+i]=(rmt_symbol_word_t){.level0=1,.duration0=one?8:4,.level1=0,.duration1=one?4:8};}
    }
    symbols[600]=(rmt_symbol_word_t){.level0=0,.duration0=3000,.level1=0,.duration1=3000};
    rmt_transmit_config_t tx={.loop_count=0};ESP_ERROR_CHECK(rmt_transmit(channel,encoder,symbols,sizeof(symbols),&tx));ESP_ERROR_CHECK(rmt_tx_wait_all_done(channel,1000));
    printf("ATOM_MATRIX_PIXELS first=%s last=blue\n",pressed?"green":"red");
}
static void read_imu(void) {
    uint8_t reg=0x3b,data[6];ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_0,0x68,&reg,1,data,6,pdMS_TO_TICKS(500)));
    printf("ATOM_MATRIX_ACCEL %d %d %d\n",(int16_t)((data[0]<<8)|data[1]),(int16_t)((data[2]<<8)|data[3]),(int16_t)((data[4]<<8)|data[5]));
}
void app_main(void) {
    gpio_config_t key={.pin_bit_mask=1ULL<<39,.mode=GPIO_MODE_INPUT};ESP_ERROR_CHECK(gpio_config(&key));
    rmt_tx_channel_config_t config={.gpio_num=27,.clk_src=RMT_CLK_SRC_DEFAULT,.resolution_hz=10000000,.mem_block_symbols=64,.trans_queue_depth=1};
    ESP_ERROR_CHECK(rmt_new_tx_channel(&config,&channel));rmt_copy_encoder_config_t copy={0};ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&encoder));ESP_ERROR_CHECK(rmt_enable(channel));
    i2c_config_t bus={.mode=I2C_MODE_MASTER,.sda_io_num=25,.scl_io_num=21,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    uint8_t reg=0x75,id=0;ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_0,0x68,&reg,1,&id,1,pdMS_TO_TICKS(500)));printf("ATOM_MATRIX_IMU_ID %02x\n",id);
    uint8_t wake[]={0x6b,1};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x68,wake,2,pdMS_TO_TICKS(500)));
    vTaskDelay(pdMS_TO_TICKS(1000));pixels(0);int last=-1,count=0;
    for(;;){int level=gpio_get_level(39);if(level!=last){printf("ATOM_MATRIX_BUTTON %d\n",level);pixels(!level);last=level;}if(count++%25==0)read_imu();fflush(stdout);vTaskDelay(pdMS_TO_TICKS(20));}
}

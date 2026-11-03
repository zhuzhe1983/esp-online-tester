// Internal IDF 5.5.1 fixture; not official firmware and never a public Pages asset.
// M5Stack StampFly schematic v1.0 + M5StampFly 2b96752c: G0 button, G21 Stamp RGB,
// G39 carrier RGB, SPI CLK44/MOSI14/MISO43/CS46, I2C3/4, motors5/42/10/41.
// Stamp-S3A v0.3.3 gates the module LED through G38. Original Stamp-S3 does not.
// Public IDF drivers own SoC registers; Bosch BMI270 config reused with its license.
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/rmt_tx.h"
#include "driver/ledc.h"
#include "driver/spi_master.h"
#include "driver/i2c.h"
#include "bmi270_config.h"

static rmt_channel_handle_t rgb_channel[2];
static rmt_encoder_handle_t rgb_encoder;
static spi_device_handle_t imu;
static const int motor_pin[4]={5,42,10,41};

static void rgb(int chain,const uint8_t *colors,int count) {
    rmt_symbol_word_t symbols[49];
    for(int led=0;led<count;led++) {
        const uint8_t *p=colors+led*3;
        uint32_t grb=((uint32_t)p[1]<<16)|((uint32_t)p[0]<<8)|p[2];
        for(int bit=0;bit<24;bit++){int one=(grb&(1U<<(23-bit)))!=0;symbols[led*24+bit]=(rmt_symbol_word_t){.level0=1,.duration0=one?8:4,.level1=0,.duration1=one?4:8};}
    }
    symbols[count*24]=(rmt_symbol_word_t){.level0=0,.duration0=3000,.level1=0,.duration1=3000};
    rmt_transmit_config_t config={.loop_count=0};
    ESP_ERROR_CHECK(rmt_transmit(rgb_channel[chain],rgb_encoder,symbols,(count*24+1)*sizeof(*symbols),&config));
    ESP_ERROR_CHECK(rmt_tx_wait_all_done(rgb_channel[chain],1000));
}
static void imu_write(uint8_t reg,const uint8_t *bytes,size_t count) {
    uint8_t tx[33]={reg};memcpy(tx+1,bytes,count);
    spi_transaction_t t={.length=(count+1)*8,.tx_buffer=tx};ESP_ERROR_CHECK(spi_device_polling_transmit(imu,&t));
}
static void imu_reg(uint8_t reg,uint8_t value){imu_write(reg,&value,1);}
static void imu_read(spi_device_handle_t device,uint8_t reg,uint8_t *bytes,size_t count) {
    // BMI270 SPI is address, one dummy byte, then register data (Bosch DS000-08 §6).
    uint8_t tx[16]={reg|0x80},rx[16]={0};
    spi_transaction_t t={.length=(count+2)*8,.tx_buffer=tx,.rx_buffer=rx};ESP_ERROR_CHECK(spi_device_polling_transmit(device,&t));memcpy(bytes,rx+2,count);
}
static esp_err_t ina_read(uint8_t address,uint8_t reg,uint16_t *value) {
    uint8_t bytes[2]={0};esp_err_t result=i2c_master_write_read_device(I2C_NUM_0,address,&reg,1,bytes,2,pdMS_TO_TICKS(100));
    *value=((uint16_t)bytes[0]<<8)|bytes[1];return result;
}
static void sensors_init(void) {
    i2c_config_t bus={.mode=I2C_MODE_MASTER,.sda_io_num=3,.scl_io_num=4,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    uint16_t manufacturer,device,voltage,wrong;
    ESP_ERROR_CHECK(ina_read(0x40,0xfe,&manufacturer));ESP_ERROR_CHECK(ina_read(0x40,0xff,&device));
    vTaskDelay(pdMS_TO_TICKS(20));ESP_ERROR_CHECK(ina_read(0x40,4,&voltage));
    printf("STAMPFLY_INA %04x %04x %u\n",manufacturer,device,voltage);
    printf("STAMPFLY_I2C_WRONG_ADDRESS %s\n",ina_read(0x41,0xfe,&wrong)==ESP_OK?"ACK":"NACK");
    spi_bus_config_t spi={.mosi_io_num=14,.miso_io_num=43,.sclk_io_num=44,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=64};
    ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST,&spi,SPI_DMA_DISABLED));
    spi_device_interface_config_t correct={.clock_speed_hz=1000000,.spics_io_num=46,.queue_size=1};ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&correct,&imu));
    spi_device_handle_t absent;spi_device_interface_config_t other=correct;other.spics_io_num=12;ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&other,&absent));
    uint8_t id;imu_read(absent,0,&id,1);printf("STAMPFLY_SPI_OTHER_CS %02x\n",id);ESP_ERROR_CHECK(spi_bus_remove_device(absent));
    imu_read(imu,0,&id,1);printf("STAMPFLY_IMU_ID %02x\n",id);
    imu_reg(0x7e,0xb6);vTaskDelay(pdMS_TO_TICKS(10));imu_read(imu,0,&id,1);
    imu_reg(0x7c,0);vTaskDelay(pdMS_TO_TICKS(10));imu_reg(0x59,0);
    for(int offset=0;offset<8192;offset+=32){imu_reg(0x5b,(offset/2)&15);imu_reg(0x5c,(offset/2)>>4);imu_write(0x5e,bmi270_config_file+offset,32);}
    imu_reg(0x59,1);vTaskDelay(pdMS_TO_TICKS(150));imu_read(imu,0x21,&id,1);printf("STAMPFLY_IMU_INIT %02x\n",id);
    imu_reg(0x41,2);imu_reg(0x43,0);imu_reg(0x7d,6);
}
static void output_step(int step) {
    uint8_t stamp[][3]={{255,0,0},{0,255,0},{0,0,255},{0,0,0}};
    uint8_t carrier[]={step?32:0,step?0:32,0,0,step?32:0,step?0:32};rgb(0,stamp[step],1);rgb(1,carrier,2);
    printf("STAMPFLY_OUTPUT %d",step);
    for(int i=0;i<4;i++){uint32_t duty=step==3?0:32*(i+1+step);ESP_ERROR_CHECK(ledc_set_duty(LEDC_LOW_SPEED_MODE,i,duty));ESP_ERROR_CHECK(ledc_update_duty(LEDC_LOW_SPEED_MODE,i));printf(" %lu",(unsigned long)duty);}puts("");fflush(stdout);
}
void app_main(void) {
    gpio_config_t key={.pin_bit_mask=1ULL<<0,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&key));
    gpio_config_t gate={.pin_bit_mask=1ULL<<38,.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&gate));ESP_ERROR_CHECK(gpio_set_level(38,0));
    rmt_copy_encoder_config_t copy={0};ESP_ERROR_CHECK(rmt_new_copy_encoder(&copy,&rgb_encoder));
    for(int i=0;i<2;i++){rmt_tx_channel_config_t config={.gpio_num=i?39:21,.clk_src=RMT_CLK_SRC_DEFAULT,.resolution_hz=10000000,.mem_block_symbols=i?96:48,.trans_queue_depth=1};ESP_ERROR_CHECK(rmt_new_tx_channel(&config,&rgb_channel[i]));ESP_ERROR_CHECK(rmt_enable(rgb_channel[i]));}
    ledc_timer_config_t timer={.speed_mode=LEDC_LOW_SPEED_MODE,.duty_resolution=LEDC_TIMER_8_BIT,.timer_num=LEDC_TIMER_0,.freq_hz=150000,.clk_cfg=LEDC_AUTO_CLK};ESP_ERROR_CHECK(ledc_timer_config(&timer));
    for(int i=0;i<4;i++){ledc_channel_config_t channel={.gpio_num=motor_pin[i],.speed_mode=LEDC_LOW_SPEED_MODE,.channel=i,.timer_sel=LEDC_TIMER_0,.duty=0};ESP_ERROR_CHECK(ledc_channel_config(&channel));}
    rgb(0,(uint8_t[]){255,0,0},1);rgb(1,(uint8_t[]){0,32,0,0,0,32},2);puts("STAMPFLY_GATE_LOW");fflush(stdout);vTaskDelay(pdMS_TO_TICKS(1000));
    ESP_ERROR_CHECK(gpio_set_level(38,1));puts("STAMPFLY_GATE_HIGH_NO_FRAME");fflush(stdout);vTaskDelay(pdMS_TO_TICKS(1000));
    sensors_init();output_step(0);puts("STAMPFLY_READY");
    int last=-1,step=0;
    for(;;) {
        int level=gpio_get_level(0);if(level!=last){printf("STAMPFLY_BUTTON %d\n",level);if(!level){step=(step+1)%4;output_step(step);}last=level;}
        uint8_t sample[12];imu_read(imu,0x0c,sample,12);
        printf("STAMPFLY_ACCEL %d %d %d\n",(int16_t)((sample[1]<<8)|sample[0]),(int16_t)((sample[3]<<8)|sample[2]),(int16_t)((sample[5]<<8)|sample[4]));
        printf("STAMPFLY_GYRO %d %d %d\n",(int16_t)((sample[7]<<8)|sample[6]),(int16_t)((sample[9]<<8)|sample[8]),(int16_t)((sample[11]<<8)|sample[10]));fflush(stdout);vTaskDelay(pdMS_TO_TICKS(100));
    }
}

// Internal IDF 5.5.1 fixture: real ADC2_CH0 conversion at StopWatch Port A white G11.
#include <stdio.h>
#include <stdint.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/uart.h"
#include "esp_adc/adc_oneshot.h"
#include "esp_adc/adc_cali.h"
#include "esp_adc/adc_cali_scheme.h"
#include "esp_system.h"
static adc_oneshot_unit_handle_t adc1,adc2;
static adc_cali_handle_t calibration;
static void power_reg(uint8_t reg,uint8_t value) {uint8_t bytes[]={reg,value};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x6e,bytes,2,pdMS_TO_TICKS(100)));}
static void sample(void) {
    int white,other,yellow,mv;ESP_ERROR_CHECK(adc_oneshot_read(adc2,ADC_CHANNEL_0,&white));ESP_ERROR_CHECK(adc_oneshot_read(adc2,ADC_CHANNEL_1,&other));ESP_ERROR_CHECK(adc_oneshot_read(adc1,ADC_CHANNEL_9,&yellow));ESP_ERROR_CHECK(adc_cali_raw_to_voltage(calibration,white,&mv));
    printf("U005_ADC white11=%d mv=%d other12=%d yellow10=%d\n",white,mv,other,yellow);fflush(stdout);
}
void app_main(void) {
    i2c_config_t bus={.mode=I2C_MODE_MASTER,.sda_io_num=47,.scl_io_num=48,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));ESP_ERROR_CHECK(uart_driver_install(UART_NUM_0,256,0,0,NULL,0));
    adc_oneshot_unit_init_cfg_t unit1={.unit_id=ADC_UNIT_1},unit2={.unit_id=ADC_UNIT_2};ESP_ERROR_CHECK(adc_oneshot_new_unit(&unit1,&adc1));ESP_ERROR_CHECK(adc_oneshot_new_unit(&unit2,&adc2));
    adc_oneshot_chan_cfg_t channel={.atten=ADC_ATTEN_DB_12,.bitwidth=ADC_BITWIDTH_12};ESP_ERROR_CHECK(adc_oneshot_config_channel(adc1,ADC_CHANNEL_9,&channel));ESP_ERROR_CHECK(adc_oneshot_config_channel(adc2,ADC_CHANNEL_0,&channel));ESP_ERROR_CHECK(adc_oneshot_config_channel(adc2,ADC_CHANNEL_1,&channel));
    adc_cali_curve_fitting_config_t cal={.unit_id=ADC_UNIT_2,.chan=ADC_CHANNEL_0,.atten=ADC_ATTEN_DB_12,.bitwidth=ADC_BITWIDTH_12};ESP_ERROR_CHECK(adc_cali_create_scheme_curve_fitting(&cal,&calibration));
    power_reg(6,0x17);puts("U005_POWER_OFF");sample();vTaskDelay(pdMS_TO_TICKS(300));power_reg(6,0x1f);puts("U005_BOOT_READY");sample();
    for(;;){uint8_t key;if(uart_read_bytes(UART_NUM_0,&key,1,pdMS_TO_TICKS(10))!=1)continue;
      switch(key){case 'q':break;case 'o':power_reg(6,0x17);puts("U005_POWER_OFF");break;case 'i':power_reg(6,0x1f);puts("U005_POWER_ON");break;case 'h':power_reg(7,0x40);puts("U005_HOLD");break;case 'n':power_reg(7,0);puts("U005_NO_HOLD");break;case 's':power_reg(0x0c,0xa1);puts("U005_SHUTDOWN");break;case 'r':puts("U005_RESTART");fflush(stdout);esp_restart();break;default:break;}sample();}
}

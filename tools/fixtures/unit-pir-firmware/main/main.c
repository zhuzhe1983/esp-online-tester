// Internal source-built ESP-IDF 5.5.1 fixture, never a public product firmware.
// U004 white J1.1 -> StopWatch Port A G11; yellow G10 is NC on the unit.
// Guest commands below change real GPIO pulls and M5PM1 registers. Output always
// comes from gpio_get_level(), never from the host's requested motion input.
#include <stdio.h>
#include <stdint.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/i2c.h"
#include "driver/uart.h"
#include "esp_system.h"
#include "esp_timer.h"

static void power_reg(uint8_t reg,uint8_t value) {
    uint8_t bytes[]={reg,value};
    ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x6e,bytes,2,pdMS_TO_TICKS(100)));
}
void app_main(void) {
    gpio_config_t signal={.pin_bit_mask=1ULL<<11,.mode=GPIO_MODE_INPUT,.pull_down_en=GPIO_PULLDOWN_ENABLE};ESP_ERROR_CHECK(gpio_config(&signal));
    gpio_config_t unused={.pin_bit_mask=1ULL<<10,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&unused));
    i2c_config_t bus={.mode=I2C_MODE_MASTER,.sda_io_num=47,.scl_io_num=48,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    ESP_ERROR_CHECK(uart_driver_install(UART_NUM_0,256,0,0,NULL,0));
    power_reg(0x06,0x17);printf("U004_POWER_OFF G11=%d G10=%d\n",gpio_get_level(11),gpio_get_level(10));fflush(stdout);
    vTaskDelay(pdMS_TO_TICKS(300));power_reg(0x06,0x1f);puts("U004_POWER_ON");
    int last=-1;
    for(;;) {
        uint8_t key;
        if(uart_read_bytes(UART_NUM_0,&key,1,0)==1) {
            switch(key) {
                case 'o':power_reg(0x06,0x17);puts("U004_COMMAND POWER_OFF");break;
                case 'i':power_reg(0x06,0x1f);puts("U004_COMMAND POWER_ON");break;
                case 'u':gpio_set_pull_mode(11,GPIO_PULLUP_ONLY);puts("U004_COMMAND PULL_UP");break;
                case 'd':gpio_set_pull_mode(11,GPIO_PULLDOWN_ONLY);puts("U004_COMMAND PULL_DOWN");break;
                case 'h':power_reg(0x07,0x40);puts("U004_COMMAND HOLD_5V");break;
                case 'n':power_reg(0x07,0);puts("U004_COMMAND NO_HOLD");break;
                case 's':power_reg(0x0c,0xa1);puts("U004_COMMAND SHUTDOWN");break;
                case 'q':puts("U004_COMMAND SAMPLE");break;
                case 'r':puts("U004_COMMAND RESTART");fflush(stdout);esp_restart();break;
                default:break;
            }
            last=-1;
        }
        int level=gpio_get_level(11);
        if(level!=last){printf("U004_READ G11=%d G10=%d US=%lld\n",level,gpio_get_level(10),(long long)esp_timer_get_time());fflush(stdout);last=level;}
        vTaskDelay(pdMS_TO_TICKS(10));
    }
}

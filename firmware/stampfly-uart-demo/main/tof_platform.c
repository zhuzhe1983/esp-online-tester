// SPDX-License-Identifier: MIT
// Independently authored ESP-IDF transport for the unmodified BSD-3-Clause ST algorithm.
#include "vl53lx_platform.h"
#include "driver/i2c.h"
#include "esp_timer.h"
#include "esp_rom_sys.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
VL53LX_Error VL53LX_WriteMulti(VL53LX_DEV d,uint16_t r,uint8_t *p,uint32_t n){
    i2c_cmd_handle_t cmd=i2c_cmd_link_create();if(!cmd)return -13;
    i2c_master_start(cmd);i2c_master_write_byte(cmd,d->i2c_slave_address<<1,true);i2c_master_write_byte(cmd,r>>8,true);i2c_master_write_byte(cmd,r,true);i2c_master_write(cmd,p,n,true);i2c_master_stop(cmd);
    esp_err_t result=i2c_master_cmd_begin(I2C_NUM_0,cmd,pdMS_TO_TICKS(30));i2c_cmd_link_delete(cmd);return result==ESP_OK?0:-13;
}
VL53LX_Error VL53LX_ReadMulti(VL53LX_DEV d,uint16_t r,uint8_t *p,uint32_t n){uint8_t address[]={r>>8,r};return i2c_master_write_read_device(I2C_NUM_0,d->i2c_slave_address,address,2,p,n,pdMS_TO_TICKS(30))==ESP_OK?0:-13;}
VL53LX_Error VL53LX_WrByte(VL53LX_DEV d,uint16_t r,uint8_t x){return VL53LX_WriteMulti(d,r,&x,1);}
VL53LX_Error VL53LX_WrWord(VL53LX_DEV d,uint16_t r,uint16_t x){uint8_t b[]={x>>8,x};return VL53LX_WriteMulti(d,r,b,2);}
VL53LX_Error VL53LX_WrDWord(VL53LX_DEV d,uint16_t r,uint32_t x){uint8_t b[]={x>>24,x>>16,x>>8,x};return VL53LX_WriteMulti(d,r,b,4);}
VL53LX_Error VL53LX_RdByte(VL53LX_DEV d,uint16_t r,uint8_t *x){return VL53LX_ReadMulti(d,r,x,1);}
VL53LX_Error VL53LX_RdWord(VL53LX_DEV d,uint16_t r,uint16_t *x){uint8_t b[2]={0};int s=VL53LX_ReadMulti(d,r,b,2);*x=(b[0]<<8)|b[1];return s;}
VL53LX_Error VL53LX_RdDWord(VL53LX_DEV d,uint16_t r,uint32_t *x){uint8_t b[4]={0};int s=VL53LX_ReadMulti(d,r,b,4);*x=((uint32_t)b[0]<<24)|(b[1]<<16)|(b[2]<<8)|b[3];return s;}
VL53LX_Error VL53LX_WaitUs(VL53LX_DEV d,int32_t us){(void)d;if(us>0)esp_rom_delay_us(us);return 0;}
VL53LX_Error VL53LX_WaitMs(VL53LX_DEV d,int32_t ms){(void)d;if(ms>0)vTaskDelay(pdMS_TO_TICKS(ms));return 0;}
VL53LX_Error VL53LX_GetTickCount(VL53LX_DEV d,uint32_t *x){(void)d;*x=esp_timer_get_time()/1000;return 0;}
VL53LX_Error VL53LX_GetTimerFrequency(int32_t *x){*x=1000000;return 0;}
VL53LX_Error VL53LX_GetTimerValue(int32_t *x){*x=(uint32_t)esp_timer_get_time();return 0;}
VL53LX_Error VL53LX_WaitValueMaskEx(VL53LX_DEV d,uint32_t timeout,uint16_t r,uint8_t v,uint8_t m,uint32_t delay){int64_t end=esp_timer_get_time()+timeout*1000LL;do{uint8_t x;int s=VL53LX_RdByte(d,r,&x);if(s)return s;if((x&m)==v)return 0;VL53LX_WaitMs(d,delay?delay:1);}while(esp_timer_get_time()<=end);return -7;}

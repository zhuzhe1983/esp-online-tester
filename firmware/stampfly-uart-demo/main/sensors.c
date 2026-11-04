// SPDX-License-Identifier: MIT
// Physical routes and FRD mounting transform from M5StampFly 2b96752c72c465c63b260996046c71c7675f81b2.
// BMI270 raw protocol (Bosch DS000-08); exact BSD configuration array and ST API 1.2.8 algorithms retained separately.
#include <stdio.h>
#include <math.h>
#include <string.h>
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "driver/i2c.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_timer.h"
#include "bmi270_config.h"
#include "vl53lx_api.h"
#include "vl53lx_api_core.h"
#include "vl53lx_core.h"
#include "sensors.h"
static spi_device_handle_t imu;
static VL53LX_Dev_t tof;
static bool imu_started,tof_started,attitude_started;
static int64_t last_imu,last_tof,last_tof_poll;
static unsigned imu_reads,tof_frames;
static int tof_status=-1,tof_mm=-1;
static float height_previous;
static esp_err_t imu_write(uint8_t reg,const uint8_t *bytes,size_t count){uint8_t tx[33]={reg};memcpy(tx+1,bytes,count);spi_transaction_t t={.length=(count+1)*8,.tx_buffer=tx};return spi_device_polling_transmit(imu,&t);}
static esp_err_t imu_reg(uint8_t r,uint8_t v){return imu_write(r,&v,1);}
static esp_err_t imu_read(uint8_t reg,uint8_t *bytes,size_t count){uint8_t tx[16]={reg|0x80},rx[16]={0};spi_transaction_t t={.length=(count+2)*8,.tx_buffer=tx,.rx_buffer=rx};esp_err_t result=spi_device_polling_transmit(imu,&t);memcpy(bytes,rx+2,count);return result;}
void sf_sensors_init(void){
    spi_bus_config_t spi={.mosi_io_num=14,.miso_io_num=43,.sclk_io_num=44,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=64};
    if(spi_bus_initialize(SPI2_HOST,&spi,SPI_DMA_DISABLED)==ESP_OK){
        spi_device_interface_config_t config={.clock_speed_hz=1000000,.spics_io_num=46,.queue_size=1};
        if(spi_bus_add_device(SPI2_HOST,&config,&imu)==ESP_OK){
            uint8_t id=0;imu_read(0,&id,1);imu_reg(0x7e,0xb6);vTaskDelay(pdMS_TO_TICKS(10));imu_read(0,&id,1);
            bool ok=id==0x24;ok&=imu_reg(0x7c,0)==ESP_OK;vTaskDelay(pdMS_TO_TICKS(10));ok&=imu_reg(0x59,0)==ESP_OK;
            for(int n=0;n<8192;n+=32){ok&=imu_reg(0x5b,(n/2)&15)==ESP_OK;ok&=imu_reg(0x5c,(n/2)>>4)==ESP_OK;ok&=imu_write(0x5e,bmi270_config_file+n,32)==ESP_OK;}
            ok&=imu_reg(0x59,1)==ESP_OK;vTaskDelay(pdMS_TO_TICKS(150));ok&=imu_read(0x21,&id,1)==ESP_OK;ok&=(id&15)==1;
            ok&=imu_reg(0x40,0xa9)==ESP_OK;ok&=imu_reg(0x41,2)==ESP_OK;ok&=imu_reg(0x42,0xe9)==ESP_OK;ok&=imu_reg(0x43,0)==ESP_OK;ok&=imu_reg(0x7d,6)==ESP_OK;imu_started=ok;
        }
    }
    i2c_config_t bus={.mode=I2C_MODE_MASTER,.sda_io_num=3,.scl_io_num=4,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=400000};
    if(i2c_param_config(I2C_NUM_0,&bus)==ESP_OK&&i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0)==ESP_OK){
        // Keep front sensor off: this demo uses bottom ranging only, no obstacle avoidance claim.
        gpio_config_t shut={.pin_bit_mask=(1ULL<<7)|(1ULL<<9),.mode=GPIO_MODE_OUTPUT};gpio_config(&shut);gpio_set_level(7,0);gpio_set_level(9,0);vTaskDelay(pdMS_TO_TICKS(2));gpio_set_level(7,1);
        tof.i2c_slave_address=0x29;tof.comms_speed_khz=400;
        int e=VL53LX_WaitDeviceBooted(&tof);if(!e)e=VL53LX_DataInit(&tof);if(!e)e=VL53LX_SetDistanceMode(&tof,VL53LX_DISTANCEMODE_MEDIUM);if(!e)e=VL53LX_SetMeasurementTimingBudgetMicroSeconds(&tof,33000);
        if(!e)e=VL53LX_ClearInterruptAndStartMeasurement(&tof);
        vTaskDelay(pdMS_TO_TICKS(100));if(!e)e=VL53LX_StartMeasurement(&tof);tof_started=e==0;
        printf("SF1 SENSOR_INIT BMI270=%d VL53L3CX=%d code=%d\n",imu_started,tof_started,e);
    }
}
static int16_t signed_le(const uint8_t *p){return (int16_t)((p[1]<<8)|p[0]);}
void sf_sensors_update(sf_sample *s,float dt,int64_t now){
    uint8_t raw[12],status=0;
    if(imu_started&&imu_read(3,&status,1)==ESP_OK&&(status&0xc0)==0xc0&&imu_read(0x0c,raw,12)==ESP_OK){
        // Sensor axes -> FRD: x=rawY, y=rawX, z=-rawZ. Static specific force z=-1g.
        float ax=signed_le(raw+2)/4096.f,ay=signed_le(raw)/4096.f,az=-signed_le(raw+4)/4096.f;
        s->p=signed_le(raw+8)/16.384f;s->q=signed_le(raw+6)/16.384f;s->r=-signed_le(raw+10)/16.384f;
        float roll=atan2f(-ay,-az)*57.2957795f,pitch=atan2f(ax,sqrtf(ay*ay+az*az))*57.2957795f;
        if(!attitude_started){s->roll=roll;s->pitch=pitch;attitude_started=true;}
        else {float correction=fminf(.02f,dt*2);s->roll=(s->roll+s->p*dt)*(1-correction)+roll*correction;s->pitch=(s->pitch+s->q*dt)*(1-correction)+pitch*correction;}
        last_imu=now;imu_reads++;
    }
    if(tof_started&&now-last_tof_poll>=5000){
        last_tof_poll=now;uint8_t ready=0;
        if(!VL53LX_GetMeasurementDataReady(&tof,&ready)&&ready){
            VL53LX_MultiRangingData_t data={0};int e=VL53LX_GetMultiRangingData(&tof,&data);if(!e)e=VL53LX_check_ll_driver_rd_state(&tof);
            tof_frames++;tof_status=e?e:data.NumberOfObjectsFound?data.RangeData[0].RangeStatus:255;tof_mm=data.NumberOfObjectsFound?data.RangeData[0].RangeMilliMeter:-1;
            if(!e&&data.NumberOfObjectsFound&&tof_status==VL53LX_RANGESTATUS_RANGE_VALID&&tof_mm>=0&&tof_mm<=3000){
                float height=fmaxf(0,tof_mm*.001f*cosf(s->roll*.0174532925f)*cosf(s->pitch*.0174532925f)-.035f);
                if(last_tof&&now-last_tof<350000){float velocity=(height-height_previous)/((now-last_tof)*1e-6f);s->vertical_speed+=.2f*(velocity-s->vertical_speed);}else s->vertical_speed=0;
                s->height=height;height_previous=height;last_tof=now;
            }
            VL53LX_ClearInterruptAndStartMeasurement(&tof);
        }
    }
    s->imu_valid=last_imu&&now-last_imu<50000;s->tof_valid=last_tof&&now-last_tof<350000;
}
void sf_sensors_report(void){printf("SF1 SENSOR imu_reads=%u tof_frames=%u tof_status=%d tof_mm=%d\n",imu_reads,tof_frames,tof_status,tof_mm);}

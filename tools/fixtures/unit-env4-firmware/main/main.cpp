// Source-only IDF 5.5.1 test client. SHT40 CRC/conversion from Sensirion D1 v2;
// BMP280 compensation uses the unchanged MIT M5Unit-ENV header pinned in upstream/.
#include <cstdio>
#include <cstdint>
#include <cstring>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/uart.h"
#include "esp_system.h"
#include "upstream/unit_BMP280_data.hpp"
static bool external_installed=false;
static void setup_external(int sda,int scl) {
    if(external_installed)ESP_ERROR_CHECK(i2c_driver_delete(I2C_NUM_1));
    i2c_config_t bus{};bus.mode=I2C_MODE_MASTER;bus.sda_io_num=sda;bus.scl_io_num=scl;bus.sda_pullup_en=GPIO_PULLUP_ENABLE;bus.scl_pullup_en=GPIO_PULLUP_ENABLE;bus.master.clk_speed=100000;
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_1,&bus));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_1,I2C_MODE_MASTER,0,0,0));external_installed=true;
    printf("ENV4_ROUTE sda=%d scl=%d\n",sda,scl);
}
static esp_err_t write_bytes(uint8_t addr,const uint8_t*data,size_t len){return i2c_master_write_to_device(I2C_NUM_1,addr,data,len,pdMS_TO_TICKS(100));}
static esp_err_t read_bytes(uint8_t addr,uint8_t*data,size_t len){return i2c_master_read_from_device(I2C_NUM_1,addr,data,len,pdMS_TO_TICKS(100));}
static esp_err_t reg_read(uint8_t reg,uint8_t*data,size_t len){return i2c_master_write_read_device(I2C_NUM_1,0x76,&reg,1,data,len,pdMS_TO_TICKS(100));}
static void power(bool on){uint8_t data[]={6,uint8_t(on?0x1f:0x17)};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x6e,data,2,pdMS_TO_TICKS(100)));printf("ENV4_POWER %d\n",on);}
static uint8_t crc(const uint8_t* data){uint8_t c=0xff;for(int i=0;i<2;i++){c^=data[i];for(int bit=0;bit<8;bit++)c=(c&0x80)?uint8_t((c<<1)^0x31):uint8_t(c<<1);}return c;}
static void probe(void){uint8_t cmd=0x89,id=0;const auto sht=write_bytes(0x44,&cmd,1),bmp=reg_read(0xd0,&id,1);printf("ENV4_PROBE sht=%d bmp=%d id=%02x\n",sht==ESP_OK,bmp==ESP_OK,id);}
static void sample(void){
    uint8_t command=0xfd,sht[6]{},raw[6]{},id=0;const auto begin=write_bytes(0x44,&command,1);vTaskDelay(pdMS_TO_TICKS(20));const auto sht_result=begin==ESP_OK?read_bytes(0x44,sht,6):begin;
    m5::unit::bmp280::Trimming trimming{};
    auto bmp_result=reg_read(0xd0,&id,1);if(bmp_result==ESP_OK)bmp_result=reg_read(0x88,trimming.value,24);
    const uint8_t measure[]={0xf4,0x25};if(bmp_result==ESP_OK)bmp_result=write_bytes(0x76,measure,2);vTaskDelay(pdMS_TO_TICKS(20));if(bmp_result==ESP_OK)bmp_result=reg_read(0xf7,raw,6);
    if(sht_result!=ESP_OK||bmp_result!=ESP_OK){printf("ENV4_NACK sht=%d bmp=%d\n",sht_result,bmp_result);fflush(stdout);return;}
    m5::unit::bmp280::Data data{};for(size_t i=0;i<6;i++)data.raw[i]=raw[i];data.trimming=&trimming;
    const float t=-45.0f+175.0f*float((sht[0]<<8)|sht[1])/65535.0f,h=-6.0f+125.0f*float((sht[3]<<8)|sht[4])/65535.0f;
    printf("ENV4_SAMPLE shtT=%.3f rh=%.3f bmpT=%.3f pressure=%.3f crc=%d id=%02x raw=%02x%02x%02x%02x%02x%02x\n",double(t),double(h),double(data.celsius()),double(data.pressure()/100),crc(sht)==sht[2]&&crc(sht+3)==sht[5],id,raw[0],raw[1],raw[2],raw[3],raw[4],raw[5]);fflush(stdout);
}
static void timing(void){uint8_t cmd=0xfd,data[6]{};auto write=write_bytes(0x44,&cmd,1);auto early=read_bytes(0x44,data,6);vTaskDelay(pdMS_TO_TICKS(20));auto ready=read_bytes(0x44,data,6);auto twice=read_bytes(0x44,data,6);printf("ENV4_TIMING write=%d early=%d ready=%d twice=%d crc=%d\n",write,early,ready,twice,crc(data)==data[2]);}
extern "C" void app_main(void){
    i2c_config_t bus{};bus.mode=I2C_MODE_MASTER;bus.sda_io_num=47;bus.scl_io_num=48;bus.sda_pullup_en=GPIO_PULLUP_ENABLE;bus.scl_pullup_en=GPIO_PULLUP_ENABLE;bus.master.clk_speed=100000;
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));ESP_ERROR_CHECK(uart_driver_install(UART_NUM_0,256,0,0,nullptr,0));setup_external(10,11);
    power(false);probe();vTaskDelay(pdMS_TO_TICKS(150));power(true);vTaskDelay(pdMS_TO_TICKS(20));puts("ENV4_BOOT_READY");sample();
    for(;;){uint8_t key;if(uart_read_bytes(UART_NUM_0,&key,1,pdMS_TO_TICKS(10))!=1)continue;
        switch(key){case 'q':sample();break;case 'p':probe();break;case 'o':power(false);break;case 'i':power(true);break;case 'w':setup_external(11,10);break;case 'c':setup_external(10,11);break;case 't':timing();break;
        case 'a':{uint8_t cmd=0x89;printf("ENV4_OTHER sht45=%d bmp77=%d\n",write_bytes(0x45,&cmd,1),write_bytes(0x77,&cmd,1));break;}
        case 'x':{uint8_t cmd=0x94,reset[]={0xe0,0xb6};printf("ENV4_RESET sht=%d bmp=%d\n",write_bytes(0x44,&cmd,1),write_bytes(0x76,reset,2));break;}
        case 'r':puts("ENV4_RESTART");fflush(stdout);esp_restart();break;default:break;}fflush(stdout);
    }
}

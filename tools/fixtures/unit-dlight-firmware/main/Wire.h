#pragma once
// Only the public TwoWire operations used by the unchanged manufacturer driver.
// Transactions use ESP-IDF's actual hardware I2C controller; no sensor response shim.
#include <cstdint>
#include <cstddef>
#include <array>
#include "driver/i2c.h"
extern esp_err_t wireError;
class TwoWire {
    std::array<uint8_t,32> tx{},rx{};size_t length=0,at=0,count=0;uint8_t address=0;bool installed=false;
public:
    void begin(int sda,int scl,uint32_t frequency) {
        if(installed)i2c_driver_delete(I2C_NUM_1);
        i2c_config_t cfg{};cfg.mode=I2C_MODE_MASTER;cfg.sda_io_num=sda;cfg.scl_io_num=scl;cfg.sda_pullup_en=GPIO_PULLUP_ENABLE;cfg.scl_pullup_en=GPIO_PULLUP_ENABLE;cfg.master.clk_speed=frequency;
        ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_1,&cfg));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_1,I2C_MODE_MASTER,0,0,0));installed=true;
    }
    void beginTransmission(uint8_t value) {address=value;length=0;}
    size_t write(uint8_t value){if(length==tx.size())return 0;tx[length++]=value;return 1;}
    size_t write(uint8_t *data,size_t size){size_t n=0;for(;n<size&&length<tx.size();n++)write(data[n]);return n;}
    int endTransmission(){wireError=i2c_master_write_to_device(I2C_NUM_1,address,tx.data(),length,pdMS_TO_TICKS(100));return wireError;}
    size_t requestFrom(uint8_t value,size_t size){if(size>rx.size())size=rx.size();wireError=i2c_master_read_from_device(I2C_NUM_1,value,rx.data(),size,pdMS_TO_TICKS(100));count=wireError==ESP_OK?size:0;at=0;return count;}
    int read(){return at<count?rx[at++]:0xff;}
};
extern TwoWire Wire;

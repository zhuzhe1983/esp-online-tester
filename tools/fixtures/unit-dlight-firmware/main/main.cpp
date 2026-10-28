#include <cstdio>
#include <cmath>
#include "driver/i2c.h"
#include "driver/uart.h"
#include "esp_system.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "M5_DLight.h"
TwoWire Wire;esp_err_t wireError=ESP_OK;static M5_DLight light;static uint8_t mode=0x10,mt=69;
static void wait_ms(int ms){vTaskDelay(pdMS_TO_TICKS(ms));}
static esp_err_t write1(uint8_t b){return i2c_master_write_to_device(I2C_NUM_1,0x23,&b,1,pdMS_TO_TICKS(100));}
static esp_err_t read2(uint16_t &value){uint8_t b[2]{};auto e=i2c_master_read_from_device(I2C_NUM_1,0x23,b,2,pdMS_TO_TICKS(100));value=uint16_t(b[0])<<8|b[1];return e;}
static void supply(bool on){uint8_t b[]={6,uint8_t(on?0x1f:0x17)};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x6e,b,2,pdMS_TO_TICKS(100)));printf("DLIGHT_POWER %u\n",on);}
static void sample(){uint16_t raw=0;auto error=read2(raw);auto vendor=light.getLUX();double lux=raw/1.2*69/mt/(mode%16==1?2:1);printf("DLIGHT_SAMPLE err=%d raw=%u vendor=%u lux=%.4f mode=%02x mt=%u\n",error,raw,vendor,lux,mode,mt);}
static void probe(uint8_t address){auto cmd=i2c_cmd_link_create();i2c_master_start(cmd);i2c_master_write_byte(cmd,address<<1,true);i2c_master_stop(cmd);auto error=i2c_master_cmd_begin(I2C_NUM_1,cmd,pdMS_TO_TICKS(100));i2c_cmd_link_delete(cmd);printf("DLIGHT_PROBE address=%02x err=%d\n",address,error);}
static void measure(uint8_t value,int ms){mode=value;light.setMode(mode);wait_ms(ms);sample();}
static void set_mt(uint8_t value){ESP_ERROR_CHECK(write1(0x40|(value>>5)));ESP_ERROR_CHECK(write1(0x60|(value&31)));mt=value;}
static uint8_t crc(uint8_t a,uint8_t b){uint8_t c=0xff;for(auto x:{a,b}){c^=x;for(int i=0;i<8;i++)c=c&0x80?(c<<1)^0x31:c<<1;}return c;}
static void environment(){uint8_t command=0xfd,data[6]{};auto s=i2c_master_write_to_device(I2C_NUM_1,0x44,&command,1,pdMS_TO_TICKS(100));wait_ms(20);auto r=i2c_master_read_from_device(I2C_NUM_1,0x44,data,6,pdMS_TO_TICKS(100));uint8_t reg=0xd0,id=0;auto b=i2c_master_write_read_device(I2C_NUM_1,0x76,&reg,1,&id,1,pdMS_TO_TICKS(100));printf("DLIGHT_ENV sht=%d/%d t=%.3f rh=%.3f crc=%d bmp=%d id=%02x\n",s,r,-45+175.0*(data[0]*256+data[1])/65535,-6+125.0*(data[3]*256+data[4])/65535,crc(data[0],data[1])==data[2]&&crc(data[3],data[4])==data[5],b,id);}
extern "C" void app_main(){
    setvbuf(stdout,nullptr,_IONBF,0);uart_config_t uart{};uart.baud_rate=115200;uart.data_bits=UART_DATA_8_BITS;uart.parity=UART_PARITY_DISABLE;uart.stop_bits=UART_STOP_BITS_1;uart.source_clk=UART_SCLK_DEFAULT;ESP_ERROR_CHECK(uart_param_config(UART_NUM_0,&uart));ESP_ERROR_CHECK(uart_driver_install(UART_NUM_0,2048,0,0,nullptr,0));
    i2c_config_t power{};power.mode=I2C_MODE_MASTER;power.sda_io_num=47;power.scl_io_num=48;power.sda_pullup_en=GPIO_PULLUP_ENABLE;power.scl_pullup_en=GPIO_PULLUP_ENABLE;power.master.clk_speed=400000;ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&power));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    supply(true);light.begin(&Wire,10,11);measure(0x10,220);puts("DLIGHT_READY");
    while(true){uint8_t c=0;if(uart_read_bytes(UART_NUM_0,&c,1,pdMS_TO_TICKS(20))<=0)continue;
        switch(c){
            case 'q':sample();break;case 'p':probe(0x23);break;case 'a':probe(0x5c);break;case 'o':supply(false);break;case 'i':supply(true);mt=69;mode=0x10;break;
            case 'w':Wire.begin(14,15,400000);probe(0x23);break;case 'c':Wire.begin(10,11,400000);probe(0x23);break;
            case 'h':set_mt(69);measure(0x20,220);break;case '2':set_mt(69);measure(0x21,220);break;case 'l':set_mt(69);measure(0x23,40);break;
            case 'm':set_mt(138);measure(0x20,400);break;case 'n':set_mt(31);measure(0x20,120);break;case 'g':set_mt(69);measure(0x10,220);break;case 'H':set_mt(69);measure(0x11,220);break;case 'L':set_mt(69);measure(0x13,40);break;
            case 'd':light.powerOff();sample();break;case 'z':light.powerOn();ESP_ERROR_CHECK(write1(7));sample();break;
            case 't':light.powerOn();ESP_ERROR_CHECK(write1(7));set_mt(69);mode=0x20;ESP_ERROR_CHECK(write1(mode));sample();wait_ms(220);sample();break;
            case 'e':environment();break;case 'r':puts("DLIGHT_REBOOT");esp_restart();break;default:break;
        }
    }
}

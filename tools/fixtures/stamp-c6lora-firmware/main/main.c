// Internal protocol fixture, NOT manufacturer factory firmware.
// Pins: S012 Sch_M5_C6_Lora v0.2.3 pp1–2 and M5Stack Arduino C6LoRa example.
// SX1262 commands: Semtech DS.SX1261-2.W.APP Rev2.2 sections13.1–13.6.
#include <stdio.h>
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/spi_master.h"
#include "driver/gpio.h"
#include "driver/uart.h"
#include "esp_system.h"
static spi_device_handle_t radio;
static void io_write(uint8_t reg,uint8_t value) {uint8_t bytes[]={reg,value};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x43,bytes,2,pdMS_TO_TICKS(100)));}
static uint8_t io_read(uint8_t reg) {uint8_t value;ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_0,0x43,&reg,1,&value,1,pdMS_TO_TICKS(100)));return value;}
static void wait_ready(void) {for(int n=0;gpio_get_level(19);n++) {if(n>1000) {printf("C6LORA_BUSY_TIMEOUT\n");abort();}vTaskDelay(1);}}
static void transfer(int cs,const uint8_t *tx,uint8_t *rx,size_t n) {spi_transaction_t t={.length=n*8,.tx_buffer=tx,.rx_buffer=rx};gpio_set_level(cs,0);ESP_ERROR_CHECK(spi_device_polling_transmit(radio,&t));gpio_set_level(cs,1);}
static void command(const uint8_t *tx,size_t n) {uint8_t rx[260];wait_ready();transfer(23,tx,rx,n);}
#define CMD(...) do {const uint8_t bytes[]={__VA_ARGS__};command(bytes,sizeof(bytes));} while(0)
static uint16_t irq(void) {uint8_t tx[]={0x12,0,0,0},rx[4];wait_ready();transfer(23,tx,rx,4);return ((uint16_t)rx[2]<<8)|rx[3];}
static void receive(void) {CMD(0x02,0xff,0xff);CMD(0x82,0xff,0xff,0xff);wait_ready();printf("C6LORA_RX_READY\n");}
void app_main(void) {
    setvbuf(stdout,NULL,_IONBF,0);
    i2c_config_t config={.mode=I2C_MODE_MASTER,.sda_io_num=10,.scl_io_num=8,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=400000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&config));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    printf("C6LORA_IO_ID %02x\n",io_read(1));uint8_t reg=1,value=0;
    printf("C6LORA_WRONG_ADDRESS %s\n",i2c_master_write_read_device(I2C_NUM_0,0x44,&reg,1,&value,1,pdMS_TO_TICKS(100))==ESP_OK?"ACK":"NACK");
    ESP_ERROR_CHECK(i2c_set_pin(I2C_NUM_0,11,8,GPIO_PULLUP_ENABLE,GPIO_PULLUP_ENABLE,I2C_MODE_MASTER));
    printf("C6LORA_WRONG_SDA %s\n",i2c_master_write_read_device(I2C_NUM_0,0x43,&reg,1,&value,1,pdMS_TO_TICKS(100))==ESP_OK?"ACK":"NACK");
    ESP_ERROR_CHECK(i2c_set_pin(I2C_NUM_0,10,8,GPIO_PULLUP_ENABLE,GPIO_PULLUP_ENABLE,I2C_MODE_MASTER));
    io_write(3,0xe0);io_write(7,0x1f);io_write(5,0x60);printf("C6LORA_RESET_HOLD\n");vTaskDelay(pdMS_TO_TICKS(100));io_write(5,0xe0);vTaskDelay(pdMS_TO_TICKS(10));printf("C6LORA_RF_ENABLED\n");
    gpio_config_t outputs={.pin_bit_mask=(1ULL<<23)|(1ULL<<11),.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&outputs));gpio_set_level(23,1);gpio_set_level(11,1);
    gpio_config_t inputs={.pin_bit_mask=(1ULL<<19)|(1ULL<<7),.mode=GPIO_MODE_INPUT};ESP_ERROR_CHECK(gpio_config(&inputs));
    spi_bus_config_t bus={.mosi_io_num=21,.miso_io_num=22,.sclk_io_num=20,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=260};ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST,&bus,SPI_DMA_DISABLED));printf("C6LORA_SPI_INITIALIZED\n");
    spi_device_interface_config_t dev={.clock_speed_hz=1000000,.mode=0,.spics_io_num=-1,.queue_size=1};ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&dev,&radio));
    uint8_t version[]={0x1d,3,0x20,0,0,0,0,0,0,0},data[260];transfer(11,version,data,sizeof(version));printf("C6LORA_WRONG_CS %02x\n",data[4]);
    wait_ready();transfer(23,version,data,sizeof(version));printf("C6LORA_RADIO %.6s\n",data+4);
    CMD(0x80,0);CMD(0x8a,1);CMD(0x86,0x36,0x40,0,0); // 868 MHz
    CMD(0x8b,7,4,1,0); // SF7, 125kHz, CR4/5, normal rate
    CMD(0x8c,0,8,0,12,1,0);CMD(0x08,0,0x43,0,0x43,0,0,0,0);
    CMD(0x0e,0,'S','T','A','M','P','-','C','6','L','O','R','A');CMD(0x83,0,0,0);
    for(int n=0;!(irq()&1);n++){if(n>1000)abort();vTaskDelay(1);}
    printf("C6LORA_TX_DONE irq=%04x dio1=%d\n",irq(),gpio_get_level(7));receive();
    ESP_ERROR_CHECK(uart_driver_install(UART_NUM_0,256,0,0,NULL,0));
    for(;;) {
        uint16_t flags=irq();
        if(flags&2) {uint8_t size[]={0x13,0,0,0};transfer(23,size,data,4);unsigned n=data[2];uint8_t read[258]={0x1e,data[3],0};transfer(23,read,data,n+3);printf("C6LORA_RX irq=%04x dio1=%d bytes=",flags,gpio_get_level(7));for(unsigned i=0;i<n;i++)printf("%02x",data[i+3]);printf("\n");receive();}
        uint8_t key;
        if(uart_read_bytes(UART_NUM_0,&key,1,pdMS_TO_TICKS(10))==1) {
            if(key=='a') {io_write(5,0xa0);printf("C6LORA_ANT_OFF\n");}
            else if(key=='l') {io_write(5,0xc0);printf("C6LORA_LNA_OFF\n");}
            else if(key=='e') {io_write(5,0xe0);receive();}
            else if(key=='r') {io_write(5,0x60);printf("C6LORA_RESET_LOW\n");}
            else if(key=='b') {printf("C6LORA_SOFTWARE_REBOOT\n");esp_restart();}
        }
    }
}

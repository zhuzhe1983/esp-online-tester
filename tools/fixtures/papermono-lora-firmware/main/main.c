// Internal source-only ESP-IDF 5.5.1 fixture. Semtech SX1261/2 DS Rev2.2,
// PaperMono C153 schematic and official e55877dc HAL pin assignments.
#include <stdio.h>
#include <string.h>
#include <assert.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "esp_timer.h"
static spi_device_handle_t radio;
static volatile unsigned edges;
static void irq(void *arg) {(void)arg;edges++;}
static void reg(uint8_t address,uint8_t r,uint8_t value) {uint8_t data[]={r,value};ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,address,data,2,pdMS_TO_TICKS(100)));}
static void transfer(const uint8_t *tx,uint8_t *rx,size_t n) {
    int64_t deadline=esp_timer_get_time()+100000;
    while(gpio_get_level(21)) {assert(esp_timer_get_time()<deadline);vTaskDelay(1);}
    gpio_set_level(41,0);spi_transaction_t t={.length=n*8,.tx_buffer=tx,.rx_buffer=rx};ESP_ERROR_CHECK(spi_device_transmit(radio,&t));gpio_set_level(41,1);
    vTaskDelay(1);
}
static void command(const uint8_t *tx,size_t n) {uint8_t rx[260];assert(n<=sizeof(rx));transfer(tx,rx,n);}
#define CMD(...) do {const uint8_t c[]={__VA_ARGS__};command(c,sizeof(c));} while(0)
static uint8_t read_reg(uint16_t r) {uint8_t tx[]={0x1d,r>>8,r,0,0},rx[5]={0xa5,0xa5,0xa5,0xa5,0xa5};transfer(tx,rx,sizeof(tx));return rx[4];}
static uint16_t get_irq(void) {uint8_t tx[]={0x12,0,0,0},rx[4];transfer(tx,rx,4);return (uint16_t)rx[2]<<8|rx[3];}
static void wait_irq(void) {int64_t until=esp_timer_get_time()+10000000;while(!gpio_get_level(5)){assert(esp_timer_get_time()<until);vTaskDelay(1);}}
void app_main(void) {
    i2c_config_t i2c={.mode=I2C_MODE_MASTER,.sda_io_num=47,.scl_io_num=48,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&i2c));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    // Bias unconnected Lite BUSY/IRQ pins low; no nonexistent peripheral drives them.
    gpio_config_t inputs={.pin_bit_mask=(1ULL<<21)|(1ULL<<5),.mode=GPIO_MODE_INPUT,.pull_down_en=GPIO_PULLDOWN_ENABLE};ESP_ERROR_CHECK(gpio_config(&inputs));gpio_config_t cs={.pin_bit_mask=1ULL<<41,.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&cs));gpio_set_level(41,1);
    ESP_ERROR_CHECK(gpio_install_isr_service(0));ESP_ERROR_CHECK(gpio_set_intr_type(5,GPIO_INTR_POSEDGE));ESP_ERROR_CHECK(gpio_isr_handler_add(5,irq,NULL));
    spi_bus_config_t spi={.mosi_io_num=38,.miso_io_num=40,.sclk_io_num=39,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=512};ESP_ERROR_CHECK(spi_bus_initialize(SPI3_HOST,&spi,SPI_DMA_CH_AUTO));
    spi_device_interface_config_t cfg={.clock_speed_hz=8000000,.spics_io_num=-1,.queue_size=1,.flags=SPI_DEVICE_NO_DUMMY};ESP_ERROR_CHECK(spi_bus_add_device(SPI3_HOST,&cfg,&radio));
    assert(read_reg(0x320)==255);puts("LORA_FIXTURE POWER_OFF_NO_RESPONSE");
    reg(0x6e,0x16,0);reg(0x6e,0x13,0x1b);reg(0x6e,0x10,4);reg(0x6e,0x11,4);
    reg(0x4f,3,2);reg(0x4f,4,2);reg(0x4f,0x13,0);reg(0x4f,0x14,0);reg(0x4f,5,2);reg(0x4f,6,2);vTaskDelay(pdMS_TO_TICKS(20));
    uint8_t probe=0x7f,identity=0;esp_err_t nfc=i2c_master_write_read_device(I2C_NUM_0,0x50,&probe,1,&identity,1,pdMS_TO_TICKS(100));
    uint8_t tx[20]={0x1d,3,0x20,0},rx[20];memset(rx,0xa5,sizeof(rx));transfer(tx,rx,sizeof(tx));
    if(nfc!=ESP_OK){for(unsigned i=0;i<sizeof(rx);i++)assert(rx[i]==255);puts("LORA_FIXTURE LITE_ABSENT");return;}
    assert(!memcmp(rx+4,"SX1261",6));puts("LORA_FIXTURE SPI3_DMA_VERSION");
    CMD(0x80,0);CMD(0x8a,1);CMD(0x86,0x36,0x40,0,0);CMD(0x8b,7,4,1,0);CMD(0x8c,0,8,0,5,1,0);CMD(0x0d,7,0x40,0x34,0x44);CMD(0x08,3,0xff,3,0xff,0,0,0,0);CMD(0x8f,254,253);CMD(0x0e,254,'H','E','L','L','O');
    CMD(0x83,0,0,0);wait_irq();assert(get_irq()==1);assert(edges>=1);CMD(0x02,0,1);assert(gpio_get_level(5)==0);puts("LORA_FIXTURE TX_DONE_IRQ");
    CMD(0x82,0,0,64);wait_irq();assert(get_irq()==512);CMD(0x02,0xff,0xff);puts("LORA_FIXTURE NO_PEER_TIMEOUT");
    CMD(0x82,0xff,0xff,0xff);puts("LORA_FIXTURE WAIT_RX");wait_irq();assert(get_irq()==2);
    uint8_t status_tx[]={0x13,0,0,0},status_rx[4];transfer(status_tx,status_rx,4);assert(status_rx[2]==4 && status_rx[3]==253);
    uint8_t data_tx[]={0x1e,253,0,0,0,0,0},data_rx[7];transfer(data_tx,data_rx,7);assert(!memcmp(data_rx+3,"PEER",4));
    uint8_t rssi_tx[]={0x14,0,0,0,0},rssi_rx[5];transfer(rssi_tx,rssi_rx,5);assert(rssi_rx[2]==140 && rssi_rx[3]==24);puts("LORA_FIXTURE RX_BUFFER PEER RSSI=-70 SNR=6");
    reg(0x4f,6,0);vTaskDelay(1);reg(0x4f,6,2);vTaskDelay(pdMS_TO_TICKS(20));assert(read_reg(0x740)==0x14);reg(0x6e,0x11,0);assert(read_reg(0x320)==255);puts("LORA_FIXTURE RESET_POWER_GATES");puts("LORA_FIXTURE PASS_FULL");
}

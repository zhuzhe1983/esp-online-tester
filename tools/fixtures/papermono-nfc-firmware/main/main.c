// Internal ESP-IDF 5.5.1 fixture. ST DS12484 Rev 8, PaperMono NFC V0.2 schematic.
// Actual I2C/GPIO6 and SPI3 interrupt traffic; no host-manufactured scan result.
#include <stdio.h>
#include <assert.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
static void write_bytes(const uint8_t *data,size_t n) {ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x50,data,n,pdMS_TO_TICKS(100)));}
static uint8_t read_reg(uint8_t r) {uint8_t v=0,c=r|0x40;ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_0,0x50,&c,1,&v,1,pdMS_TO_TICKS(100)));return v;}
static void write_reg(uint8_t r,uint8_t v) {write_bytes((uint8_t[]){r,v},2);}
static void command(uint8_t c) {write_bytes(&c,1);}
static void clear_irq(void) {for(int i=0;i<4;i++)read_reg(0x1a+i);}
void app_main(void) {
    i2c_config_t bus={.mode=I2C_MODE_MASTER,.sda_io_num=47,.scl_io_num=48,.sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    uint8_t c=0x7f,id=0;esp_err_t result=i2c_master_write_read_device(I2C_NUM_0,0x50,&c,1,&id,1,pdMS_TO_TICKS(100));
    if(result!=ESP_OK){puts("NFC_FIXTURE LITE_NACK");return;}assert(id==0x2a);puts("NFC_FIXTURE IDENTITY 2a");
    gpio_config_t irq={.pin_bit_mask=1ULL<<6,.mode=GPIO_MODE_INPUT};ESP_ERROR_CHECK(gpio_config(&irq));
    write_reg(0x3f,0);assert(read_reg(0x3f)==0x2a);command(0xc1);write_reg(2,0xc8);vTaskDelay(pdMS_TO_TICKS(20));
    assert((read_reg(0x31)&0x10)!=0);assert(gpio_get_level(6)==1);assert(read_reg(0x1a)==0x80);assert(gpio_get_level(6)==0);puts("NFC_FIXTURE OSC_IRQ_READ_CLEAR");
    write_reg(0x16,8);write_reg(0x17,0x40);write_reg(0x10,0);write_reg(0x11,10);command(0xc6);vTaskDelay(pdMS_TO_TICKS(20));
    assert(gpio_get_level(6)==0);write_reg(0x17,0);assert(gpio_get_level(6)==1);assert(read_reg(0x1b)==0x40);assert(gpio_get_level(6)==0);assert((read_reg(0x1a)&0x38)==8);assert(read_reg(0x1e)==0);puts("NFC_FIXTURE REQA_TX_NRT_MASK_NO_RX");
    write_reg(0x16,0);write_reg(0x10,0xff);write_reg(0x11,0xff);command(0xc7);vTaskDelay(pdMS_TO_TICKS(20));clear_irq();command(0xc2);vTaskDelay(pdMS_TO_TICKS(400));assert(read_reg(0x1b)==0);puts("NFC_FIXTURE STOP_CANCELS_NRT");
    // SPI3 source 22: spi_device_transmit needs DMA_INT_SET to launch its first queued ISR.
    spi_bus_config_t spi={.mosi_io_num=38,.miso_io_num=40,.sclk_io_num=39,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=16};ESP_ERROR_CHECK(spi_bus_initialize(SPI3_HOST,&spi,SPI_DMA_DISABLED));
    spi_device_interface_config_t cfg={.clock_speed_hz=1000000,.spics_io_num=41,.queue_size=1};spi_device_handle_t dev;ESP_ERROR_CHECK(spi_bus_add_device(SPI3_HOST,&cfg,&dev));
    spi_transaction_t t={.flags=SPI_TRANS_USE_TXDATA,.length=8,.tx_data={0x00}};ESP_ERROR_CHECK(spi_device_transmit(dev,&t));puts("NFC_FIXTURE SPI3_QUEUED_COMPLETED");
    puts("NFC_FIXTURE PASS_FULL");
}

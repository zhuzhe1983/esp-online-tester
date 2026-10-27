// Internal ESP-IDF 5.5.1 test, never a factory firmware or a Pages asset.
// DinMeter schematic v1.0 and factory d2fdfe44: LCD 4/5/6/7/8,
// ESP32Encoder attachHalfQuad(40,41); independent software decoder A41/B40.
#include <stdio.h>
#include <stdint.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "driver/pcnt.h"
static spi_device_handle_t lcd;
static volatile int encoder_count,encoder_state=3;
static void encoder_edge(void *arg) {
    (void)arg;int next=gpio_get_level(41)|(gpio_get_level(40)<<1);
    const int8_t changes[]={0,-1,1,0,1,0,0,-1,-1,0,0,1,0,1,-1,0};
    encoder_count+=changes[(encoder_state<<2)|next];encoder_state=next;
}
static void transfer(int dc,const void *bytes,size_t n) {gpio_set_level(4,dc);spi_transaction_t t={.length=n*8,.tx_buffer=bytes};ESP_ERROR_CHECK(spi_device_polling_transmit(lcd,&t));}
static void command(uint8_t c,const uint8_t *data,size_t n) {transfer(0,&c,1);if(n)transfer(1,data,n);}
void app_main(void) {
    gpio_config_t out={.pin_bit_mask=(1ULL<<4)|(1ULL<<8)|(1ULL<<9)|(1ULL<<46),.mode=GPIO_MODE_OUTPUT};ESP_ERROR_CHECK(gpio_config(&out));gpio_set_level(46,1);gpio_set_level(8,0);vTaskDelay(pdMS_TO_TICKS(10));gpio_set_level(8,1);gpio_set_level(9,1);
    gpio_config_t buttons={.pin_bit_mask=1ULL<<42,.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&buttons));
    gpio_config_t enc={.pin_bit_mask=(1ULL<<41)|(1ULL<<40),.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE,.intr_type=GPIO_INTR_ANYEDGE};ESP_ERROR_CHECK(gpio_config(&enc));ESP_ERROR_CHECK(gpio_install_isr_service(0));ESP_ERROR_CHECK(gpio_isr_handler_add(41,encoder_edge,NULL));ESP_ERROR_CHECK(gpio_isr_handler_add(40,encoder_edge,NULL));
    pcnt_config_t pcnt={.pulse_gpio_num=40,.ctrl_gpio_num=41,.lctrl_mode=PCNT_MODE_KEEP,.hctrl_mode=PCNT_MODE_REVERSE,.pos_mode=PCNT_COUNT_DEC,.neg_mode=PCNT_COUNT_INC,.counter_h_lim=32767,.counter_l_lim=-32768,.unit=PCNT_UNIT_0,.channel=PCNT_CHANNEL_0};
    ESP_ERROR_CHECK(pcnt_unit_config(&pcnt));ESP_ERROR_CHECK(pcnt_set_filter_value(PCNT_UNIT_0,250));ESP_ERROR_CHECK(pcnt_filter_enable(PCNT_UNIT_0));ESP_ERROR_CHECK(pcnt_counter_clear(PCNT_UNIT_0));ESP_ERROR_CHECK(pcnt_counter_resume(PCNT_UNIT_0));
    spi_bus_config_t bus={.mosi_io_num=5,.miso_io_num=-1,.sclk_io_num=6,.quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=64};ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST,&bus,SPI_DMA_DISABLED));
    spi_device_interface_config_t panel={.clock_speed_hz=1000000,.spics_io_num=7,.queue_size=1};ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&panel,&lcd));
    command(0x11,NULL,0);command(0x29,NULL,0);command(0x3a,(uint8_t[]){0x55},1);command(0x36,(uint8_t[]){0xb0},1);command(0x2a,(uint8_t[]){0,40,1,23},4);command(0x2b,(uint8_t[]){0,52,0,186},4);command(0x2c,NULL,0);
    uint8_t pixels[32];for(int y=0;y<135;y++){for(int x=0;x<240;x+=16){for(int i=0;i<16;i++){uint16_t c=y<67?(x+i<120?0xf800:0x07e0):(x+i<120?0x001f:0xffff);pixels[2*i]=c>>8;pixels[2*i+1]=c;}transfer(1,pixels,32);}}
    puts("DINMETER_LCD_READY 240x135 TL=red TR=green BL=blue BR=white");
    int last_button=-1,last_encoder=999,last_pcnt=999;
    for(;;){int b=gpio_get_level(42),count=encoder_count;int16_t hw;ESP_ERROR_CHECK(pcnt_get_counter_value(PCNT_UNIT_0,&hw));if(b!=last_button){printf("DINMETER_BUTTON %d\n",b);last_button=b;}if(count!=last_encoder){printf("DINMETER_ENCODER %d\n",count);last_encoder=count;}if(hw!=last_pcnt){printf("DINMETER_PCNT %d\n",hw);last_pcnt=hw;}fflush(stdout);vTaskDelay(pdMS_TO_TICKS(20));}
}

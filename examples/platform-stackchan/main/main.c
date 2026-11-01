// SPDX-License-Identifier: MIT
// Internal K151 diagnostic, ESP-IDF v5.4.3. No host-drawn pixels or servo feedback injection.
// Protocols: StackChan-BSP e2f3b02dbe7f28e26a02adc72bef3aa2c545e4a0,
// M5GFX 7cb26b44ab31bb41a0a597196dd2c6d868869a21. See README for source receipts.
#include <stdio.h>
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/i2c.h"
#include "driver/spi_master.h"
#include "driver/gpio.h"
#include "driver/uart.h"
#include "esp_timer.h"

static spi_device_handle_t lcd;
static uint16_t line[320];

static void wr(uint8_t addr, uint8_t reg, uint8_t val) {
    uint8_t data[] = {reg,val};
    ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,addr,data,sizeof(data),pdMS_TO_TICKS(100)));
}
static void rd(uint8_t addr, uint8_t reg, uint8_t *data, size_t len) {
    ESP_ERROR_CHECK(i2c_master_write_read_device(I2C_NUM_0,addr,&reg,1,data,len,pdMS_TO_TICKS(100)));
}
static void lcd_bytes(bool data, const void *bytes, size_t len) {
    if (!len) return;
    gpio_set_level(35,data);
    spi_transaction_t tx = {.length=len*8,.tx_buffer=bytes};
    ESP_ERROR_CHECK(spi_device_polling_transmit(lcd,&tx));
}
static void command(uint8_t cmd, const void *bytes, size_t len) {
    lcd_bytes(false,&cmd,1); lcd_bytes(true,bytes,len);
}
static void rect(int x, int y, int width, int height, uint16_t color) {
    uint16_t be = (color << 8) | (color >> 8);
    for (int i=0;i<width;i++) line[i]=be;
    int right=x+width-1, bottom=y+height-1;
    uint8_t cols[]={x>>8,x,right>>8,right}, rows[]={y>>8,y,bottom>>8,bottom};
    command(0x2a,cols,sizeof(cols)); command(0x2b,rows,sizeof(rows));
    uint8_t write=0x2c; lcd_bytes(false,&write,1);
    for (int i=0;i<height;i++) lcd_bytes(true,line,width*2);
}
static void rgb(uint8_t index, uint16_t color) {
    uint8_t data[]={0x30+index*2,color,color>>8};
    ESP_ERROR_CHECK(i2c_master_write_to_device(I2C_NUM_0,0x6f,data,sizeof(data),pdMS_TO_TICKS(100)));
}
static void rgb_all(uint16_t color) {
    for (uint8_t i=0;i<12;i++) rgb(i,color);
    wr(0x6f,0x24,12|0x40);
}
// SCSCL packet checksum covers ID, length, instruction and parameters. Data words are big endian.
static int servo(uint8_t id, uint8_t instruction, const uint8_t *params, size_t count, uint8_t *data, size_t size) {
    uint8_t packet[32]={0xff,0xff,id,count+2,instruction};
    if (count>sizeof(packet)-6 || size>24) return -1;
    memcpy(packet+5,params,count);
    uint8_t sum=id+count+2+instruction;
    for (size_t i=0;i<count;i++) sum+=params[i];
    packet[5+count]=~sum;
    uart_flush_input(UART_NUM_1);
    if (uart_write_bytes(UART_NUM_1,packet,count+6)!=(int)count+6) return -1;
    uint8_t reply[32]; size_t got=0, expected=size+6;
    int64_t until=esp_timer_get_time()+100000;
    while (got<expected && esp_timer_get_time()<until) {
        int n=uart_read_bytes(UART_NUM_1,reply+got,expected-got,pdMS_TO_TICKS(10));
        if (n>0) got+=n;
    }
    if (got!=expected || reply[0]!=0xff || reply[1]!=0xff || reply[2]!=id || reply[3]!=size+2 || reply[4]) return -1;
    sum=0;
    for (size_t i=2;i<expected;i++) sum+=reply[i];
    if (sum!=0xff) return -1;
    if (size) memcpy(data,reply+5,size);
    return 0;
}
static void torque(uint8_t id, bool enabled) {
    uint8_t params[]={40,enabled};
    printf("SERVO_TORQUE id=%u enabled=%d ack=%d\n",id,enabled,servo(id,3,params,sizeof(params),NULL,0));
}
static void position(uint8_t id, uint16_t raw) {
    uint8_t params[]={42,raw>>8,raw,0,200,0,0};
    printf("SERVO_GOAL id=%u raw=%u ack=%d\n",id,raw,servo(id,3,params,sizeof(params),NULL,0));
}
static int feedback(uint8_t id) {
    uint8_t params[]={56,2}, data[2];
    int result=servo(id,2,params,sizeof(params),data,sizeof(data));
    int raw=result<0 ? -1 : ((int)data[0]<<8)|data[1];
    printf("SERVO_FEEDBACK id=%u raw=%d\n",id,raw);
    return raw;
}
static void expect(const char *name, bool pass) { printf("CHECK %s %s\n",name,pass ? "PASS" : "FAIL"); }

void app_main(void) {
    setvbuf(stdout,NULL,_IONBF,0);
    i2c_config_t bus={.mode=I2C_MODE_MASTER,.sda_io_num=12,.scl_io_num=11,
        .sda_pullup_en=GPIO_PULLUP_ENABLE,.scl_pullup_en=GPIO_PULLUP_ENABLE,.master.clk_speed=100000};
    ESP_ERROR_CHECK(i2c_param_config(I2C_NUM_0,&bus));
    ESP_ERROR_CHECK(i2c_driver_install(I2C_NUM_0,I2C_MODE_MASTER,0,0,0));
    // CoreS3 reset lines: AW9523 P0_0 touch / P1_1 LCD. DLDO1 powers the backlight.
    wr(0x58,0x04,0x18); wr(0x58,0x05,0x0c); wr(0x58,0x11,0x10);
    wr(0x58,0x12,0xff); wr(0x58,0x13,0xff);
    wr(0x58,0x02,0x05); wr(0x58,0x03,0x03);
    wr(0x34,0x90,0xbf); wr(0x34,0x99,28);
    uint8_t version=0; rd(0x6f,2,&version,1);
    expect("expander-version",version!=0 && version!=0xff);
    wr(0x6f,3,1); wr(0x6f,4,0x20); wr(0x6f,9,1); wr(0x6f,0x0a,0x20);
    wr(0x6f,0x14,0); wr(0x6f,5,1); wr(0x6f,0x24,12);
    for (uint8_t reg=0x0a;reg<=0x0f;reg++) wr(0x68,reg,0);
    wr(0x68,9,0x0f); wr(0x68,9,0x07); wr(0x68,8,0x22);
    for (uint8_t reg=2;reg<=6;reg++) wr(0x68,reg,0);
    rgb_all(0x001f);
    gpio_config_t dc={.pin_bit_mask=1ULL<<35,.mode=GPIO_MODE_OUTPUT};
    ESP_ERROR_CHECK(gpio_config(&dc));
    spi_bus_config_t spi={.sclk_io_num=36,.mosi_io_num=37,.miso_io_num=-1,
        .quadwp_io_num=-1,.quadhd_io_num=-1,.max_transfer_sz=640};
    ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST,&spi,SPI_DMA_CH_AUTO));
    spi_device_interface_config_t dev={.clock_speed_hz=10000000,.mode=0,.spics_io_num=3,.queue_size=1};
    ESP_ERROR_CHECK(spi_bus_add_device(SPI2_HOST,&dev,&lcd));
    command(0x11,NULL,0); vTaskDelay(pdMS_TO_TICKS(120));
    uint8_t format=0x55, madctl=0x00;
    command(0x3a,&format,1); command(0x36,&madctl,1); command(0x21,NULL,0); command(0x29,NULL,0);
    rect(0,0,320,240,0x0842);
    rect(60,60,36,36,0xffff); rect(224,60,36,36,0xffff); rect(110,142,100,8,0xffff);
    uart_config_t serial={.baud_rate=1000000,.data_bits=UART_DATA_8_BITS,.parity=UART_PARITY_DISABLE,
        .stop_bits=UART_STOP_BITS_1,.flow_ctrl=UART_HW_FLOWCTRL_DISABLE,.source_clk=UART_SCLK_DEFAULT};
    ESP_ERROR_CHECK(uart_param_config(UART_NUM_1,&serial));
    ESP_ERROR_CHECK(uart_set_pin(UART_NUM_1,6,7,UART_PIN_NO_CHANGE,UART_PIN_NO_CHANGE));
    ESP_ERROR_CHECK(uart_driver_install(UART_NUM_1,256,0,0,NULL,0));
    torque(1,true); torque(2,true); position(1,588); position(2,748);
    vTaskDelay(pdMS_TO_TICKS(400));
    expect("yaw-goal-feedback",feedback(1)==588);
    expect("pitch-goal-feedback",feedback(2)==748);
    // Bad GPIO route and VM_EN must suppress replies from the physical servo bus.
    ESP_ERROR_CHECK(uart_set_pin(UART_NUM_1,8,7,UART_PIN_NO_CHANGE,UART_PIN_NO_CHANGE));
    expect("wrong-tx-no-response",feedback(1)==-1);
    ESP_ERROR_CHECK(uart_set_pin(UART_NUM_1,6,7,UART_PIN_NO_CHANGE,UART_PIN_NO_CHANGE));
    wr(0x6f,5,0); expect("motor-power-off-no-response",feedback(1)==-1);
    wr(0x6f,5,1); vTaskDelay(pdMS_TO_TICKS(200));
    expect("motor-power-recovery-response",feedback(1)>=0);
    torque(1,true); torque(2,true); position(1,460); position(2,620);
    printf("PLATFORM_READY stackchan; front/middle/back=Si12T channels2/1/0; touch=FT6336U\n");
    uint8_t last_pads=0xff, last_touch=0xff;
    for (;;) {
        uint8_t pads=0, touch[5]={0};
        rd(0x68,0x10,&pads,1); rd(0x38,2,touch,sizeof(touch));
        if (pads!=last_pads) {
            printf("TOUCH_PADS front=%u middle=%u back=%u raw=%u\n",(pads>>4)&3,(pads>>2)&3,pads&3,pads);
            uint16_t color=(pads&0x30) ? 0xf800 : (pads&0x0c) ? 0x07e0 : (pads&3) ? 0x001f : 0x4208;
            rgb_all(color);
            rect(20,195,280,24,color);
            position(1,(pads&0x30) ? 588 : (pads&3) ? 332 : 460);
            position(2,(pads&0x0c) ? 812 : 620);
            last_pads=pads;
        }
        if (touch[0]!=last_touch) {
            int x=((touch[1]&15)<<8)|touch[2], y=((touch[3]&15)<<8)|touch[4];
            printf("TOUCH_SCREEN count=%u x=%d y=%d\n",touch[0]&15,x,y);
            rect(110,142,100,8,(touch[0]&15) ? 0xffe0 : 0xffff);
            last_touch=touch[0];
        }
        vTaskDelay(pdMS_TO_TICKS(50));
    }
}

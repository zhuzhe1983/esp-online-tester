#pragma once

#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/gpio.h"
#include "bsp_keyboard.h"

// 工作模式定义
typedef enum {
    MODE_NONE = 0,     // 未定义模式
    MODE_I2C = 1,      // I2C 模式 (默认, Fn+1)
    MODE_UART = 2,     // UART 模式 (Fn+2)
    MODE_ESPNOW = 3,   // ESP-NOW 模式 (Fn+3)
    MODE_BLEHID = 4,   // BLE HID 模式 (Fn+4)
    MODE_PRODTEST = 5, // 产测模式
    MODE_MAX
} app_mode_t;

// I2C 触发产测模式切换相关变量（供 bsp_i2c ISR 使用）
extern QueueHandle_t mode_queue;
extern app_mode_t g_mode;

#ifdef __cplusplus
extern "C" {
#endif

// 固件版本定义
#define FIRMWARE_VERSION    0x02

// 键盘状态定义
typedef struct {
    bool caps_lock;   // 大写锁定状态
    bool fn_pressed;  // Fn 键状态
    bool sym_pressed; // Sym 键状态
} keyboard_state_t;

// LED 状态定义
typedef struct {
    bool caps_led_on; // 大写锁定红灯
    bool sym_led_on;  // 符号模式绿灯  
    bool fn_led_on;   // Fn 激活蓝灯
} led_state_t;

// 按键索引定义
#define KEY_AA_TOGGLE    22 // Aa 大小写切换 (行2, 列0)
#define KEY_DEL          21 // Del 删除键 (行1, 列10)
#define KEY_ENTER        32 // Enter 回车键 (行2, 列10)
#define KEY_FN           33 // Fn 功能键 (行3, 列0)
#define KEY_SYM          34 // Sym 符号键 (行3, 列1)
#define KEY_SPACE        42 // Space 空格键 (行3, 列9)

// Fn 键组合功能定义
#define KEY_ESC          0  // ESC 键 (行0, 列0) = 0*11+0 = 0 (Fn+1)
#define KEY_UP           25 // 上方向键 (行2, 列3) = 2*11+3 = 25 (Fn+D)
#define KEY_LEFT         35 // 左方向键 (行3, 列2) = 3*11+2 = 35 (Fn+Z)
#define KEY_DOWN         36 // 下方向键 (行3, 列3) = 3*11+3 = 36 (Fn+X)
#define KEY_RIGHT        37 // 右方向键 (行3, 列4) = 3*11+4 = 37 (Fn+C)

// 输出模式选择：1=DEBUG模式(输出index), 0=正常模式(输出ASCII)
#define KEYBOARD_DEBUG_MODE   0

// 函数声明
void app_key_event_cb(button_event_data_t *event_data);
void app_mode_switch(void *arg);
void app_factory(void *arg);

#ifdef __cplusplus
}
#endif

 
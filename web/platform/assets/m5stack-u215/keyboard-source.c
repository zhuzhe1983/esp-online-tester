/*******************************************************************************
 
*******************************************************************************/
#include "bsp_keyboard.h"
#include "esp_log.h"
#include "iot_button.h"
#include "button_matrix.h"


static const char *TAG = "bsp/keyboard";


// 键盘配置，实际 (ROW1, COM10)， (ROW3, COM10) 未接按键
static const int32_t row_gpios[] = {0, 1, 2, 3};                            // ROW0-ROW3
static const int32_t col_gpios[] = {4, 5, 6, 7, 8, 29, 10, 11, 22, 23, 24}; // COM0-COM10
// 按键句柄数组 (4x11 = 44个按键)
static button_handle_t button_handles[44] = {0};
// 按键事件回调
static keyboard_event_cb_t event_cb = NULL;


static void button_event_cb(void *arg, void *data)
{
    button_event_t event = iot_button_get_event(arg);
    int button_index = (int)data;

    button_event_data_t event_data = {
        .button_index = button_index,
        .event = event
    };

    if (event_cb != NULL) {
        event_cb(&event_data);
    }
}

esp_err_t bsp_keyboard_init(void)
{
    const button_config_t btn_cfg = {
        .long_press_time = 400,
        .short_press_time = 200,
    }; 
    const button_matrix_config_t matrix_cfg = {
        .row_gpios = (int32_t*)row_gpios,
        .col_gpios = (int32_t*)col_gpios,
        .row_gpio_num = 4,
        .col_gpio_num = 11,
    };
    size_t btn_num = 44;
    
    esp_err_t ret = iot_button_new_matrix_device(&btn_cfg, &matrix_cfg, button_handles, &btn_num);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Create matrix button device failed: %s", esp_err_to_name(ret));
        return ret;
    }

    return ESP_OK;
}

esp_err_t bsp_keyboard_add_event_cb(keyboard_event_cb_t cb, button_event_t event_type)
{
    if (cb == NULL) {
        return ESP_ERR_INVALID_ARG;
    }

    if (event_cb == NULL) {
        event_cb = cb;
    }

    for (int i = 0; i < 44; i++) {
        if (button_handles[i] != NULL) {
            iot_button_register_cb(button_handles[i], event_type, NULL, button_event_cb, (void*)i);
        }
    }
    
    return ESP_OK;
}

esp_err_t bsp_keyboard_add_event_cb_by_index(int button_index, keyboard_event_cb_t cb, button_event_t event_type)
{
    if (button_index < 0 || button_index >= 44) {
        return ESP_ERR_INVALID_ARG;
    }

    if (cb == NULL) {
        return ESP_ERR_INVALID_ARG;
    }
    if (event_cb == NULL) {
        event_cb = cb;
    }

    if (button_handles[button_index] == NULL) {
        return ESP_ERR_INVALID_STATE;
    }
    
    return iot_button_register_cb(button_handles[button_index], event_type, NULL, button_event_cb, (void*)button_index);
}
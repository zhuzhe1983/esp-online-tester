// ESP-IDF 5.5.1: exercise real SPI Flash writes, NVS, wear-levelled FAT and esp_restart.
// This internal fixture is not a product firmware or a deployed website asset.
#include <stdio.h>
#include <string.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_system.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "esp_vfs_fat.h"
#include "wear_levelling.h"

void app_main(void) {
    ESP_ERROR_CHECK(nvs_flash_init());
    nvs_handle_t nvs;
    ESP_ERROR_CHECK(nvs_open("fixture", NVS_READWRITE, &nvs));
    uint32_t marker = 0;
    esp_err_t read = nvs_get_u32(nvs, "marker", &marker);
    ESP_ERROR_CHECK(read == ESP_ERR_NVS_NOT_FOUND ? ESP_OK : read);
    wl_handle_t wl = WL_INVALID_HANDLE;
    const esp_vfs_fat_mount_config_t config = {.format_if_mount_failed=true,.max_files=2,.allocation_unit_size=4096};
    ESP_ERROR_CHECK(esp_vfs_fat_spiflash_mount_rw_wl("/fixture", "storage", &config, &wl));
    if (marker == 0) {
        FILE *file = fopen("/fixture/config.txt", "wb");
        if (!file || fwrite("persistent-config-v1\n", 1, 21, file) != 21 || fclose(file)) abort();
        ESP_ERROR_CHECK(nvs_set_u32(nvs, "marker", 0x1234));
        ESP_ERROR_CHECK(nvs_commit(nvs));
        nvs_close(nvs);
        ESP_ERROR_CHECK(esp_vfs_fat_spiflash_unmount_rw_wl("/fixture", wl));
        puts("STORAGE_WRITTEN NVS FAT; SOFTWARE_RESTART"); fflush(stdout);
        esp_restart();
    }
    char text[32] = {0};
    FILE *file = fopen("/fixture/config.txt", "rb");
    if (!file || fread(text, 1, sizeof(text)-1, file) != 21 || fclose(file) || strcmp(text, "persistent-config-v1\n") || marker != 0x1234) abort();
    puts("STORAGE_RESTORED NVS FAT"); fflush(stdout);
    nvs_close(nvs);
    ESP_ERROR_CHECK(esp_vfs_fat_spiflash_unmount_rw_wl("/fixture", wl));
    while (1) vTaskDelay(pdMS_TO_TICKS(1000));
}

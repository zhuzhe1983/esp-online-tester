// Internal execution fixture, not M5Stack factory firmware. UART0 is an explicit
// logical host connection, not a button/USB/BTB peripheral on the bare module.
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <inttypes.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "driver/uart.h"
#include "esp_chip_info.h"
#include "esp_psram.h"
#include "esp_heap_caps.h"
#include "esp_system.h"

static uint32_t next(uint32_t x) { x ^= x << 13; x ^= x >> 17; return x ^ (x << 5); }
void app_main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    esp_chip_info_t chip; esp_chip_info(&chip);
    printf("P4_BOOT model=%d cores=%d revision=%d reset=%d\n", chip.model, chip.cores, chip.revision, esp_reset_reason());
    printf("P4_PSRAM bytes=%u\n", (unsigned)esp_psram_get_size());
    // Span 2 MiB, with nonzero values on every 4 KiB page and at the last word.
    const size_t words = 2 * 1024 * 1024 / sizeof(uint32_t);
    volatile uint32_t *ram = heap_caps_malloc(words * sizeof(uint32_t), MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    if (!ram) { puts("P4_ALLOC_FAILED"); return; }
    for (size_t i = 0; i < words; i += 1024) ram[i] = (uint32_t)i ^ 0x5a31e7c9;
    ram[words - 1] = 0xa1257bc3;
    for (size_t i = 0; i < words; i += 1024) if (ram[i] != ((uint32_t)i ^ 0x5a31e7c9)) { puts("P4_MEMORY_FAILED"); return; }
    if (ram[words - 1] != 0xa1257bc3) { puts("P4_MEMORY_FAILED"); return; }
    puts("P4_MEMORY_PASS bytes=2097152 stride=4096");
    ESP_ERROR_CHECK(uart_driver_install(UART_NUM_0, 256, 0, 0, NULL, 0));
    puts("P4_READY");
    char line[32]; size_t length = 0; unsigned count = 0;
    for (;;) {
        uint8_t c;
        if (uart_read_bytes(UART_NUM_0, &c, 1, pdMS_TO_TICKS(20)) != 1) continue;
        if (c == '\n') {
            line[length] = 0; length = 0;
            if (line[0] == 'B' && line[1] == 0) { puts("P4_RESTART"); vTaskDelay(pdMS_TO_TICKS(20)); esp_restart(); }
            if (line[0] != 'C' || line[1] != ':') { puts("P4_REJECT"); continue; }
            char *end; uint32_t value = strtoul(line + 2, &end, 16);
            if (*end || end == line + 2) { puts("P4_REJECT"); continue; }
            uint32_t seed = value, sum = 0;
            for (size_t i = 0; i < 4096; i++) ram[i] = seed = next(seed);
            for (size_t i = 0; i < 4096; i++) sum += ram[i];
            printf("P4_RESULT count=%u input=%08" PRIx32 " sum=%08" PRIx32 "\n", ++count, value, sum);
        } else if (length + 1 < sizeof(line)) line[length++] = (char)c;
        else { length = 0; puts("P4_REJECT"); }
    }
}

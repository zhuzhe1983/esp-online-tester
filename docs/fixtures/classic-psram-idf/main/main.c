// SPDX-License-Identifier: MIT
#include <stdint.h>
#include <stdio.h>
#include "esp_heap_caps.h"
#include "esp32/spiram.h"
#include "esp_system.h"

#define COUNT (1024u * 1024u)
static uint8_t pattern(size_t i) { return (uint8_t)((i * 13u) ^ (i >> 15)); }
void app_main(void) {
    printf("P4 profile=boot-init idf=%s\n", esp_get_idf_version());
    const size_t physical = esp_spiram_get_size();
    const size_t heap = heap_caps_get_total_size(MALLOC_CAP_SPIRAM);
    const size_t free_bytes = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
    printf("P4 physical=%u heap=%u free=%u\n", (unsigned)physical, (unsigned)heap, (unsigned)free_bytes);
    uint8_t *p = heap_caps_malloc(COUNT, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    if (!p) { puts("P4 result=allocation-failed"); return; }
    printf("P4 allocation=%u address=%08x\n", COUNT, (unsigned)(uintptr_t)p);
    const int external = (uintptr_t)p >= 0x3f800000u && (uintptr_t)p + COUNT <= 0x3fc00000u;
    for (size_t i = 0; i < COUNT; ++i) p[i] = pattern(i);
    uint32_t hash = 2166136261u;
    size_t errors = 0;
    for (size_t i = 0; i < COUNT; ++i) {
        uint8_t value = p[i];
        errors += value != pattern(i);
        hash = (hash ^ value) * 16777619u;
    }
    unsigned pages = (((uintptr_t)p + COUNT - 1) >> 15) - ((uintptr_t)p >> 15) + 1;
    printf("P4 readback=%u errors=%u fnv1a=%08x pages=%u\n", COUNT, (unsigned)errors, (unsigned)hash, pages);
    void *bad = heap_caps_malloc(1024, MALLOC_CAP_SPIRAM | MALLOC_CAP_DMA);
    void *dma = heap_caps_malloc(1024, MALLOC_CAP_DMA);
    int internal = dma && (uintptr_t)dma >= 0x3ffae000u && (uintptr_t)dma < 0x40000000u;
    printf("P4 spiram_dma=%s plain_dma=%s\n", bad ? "unexpected" : "rejected", internal ? "internal" : "failed");
    void *exhausted = heap_caps_malloc(5 * COUNT, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    printf("P4 exhaustion=%s\n", exhausted ? "unexpected" : "rejected");
    heap_caps_free(p);
    heap_caps_free(bad);
    heap_caps_free(dma);
    heap_caps_free(exhausted);
    // IDF v4.4.7 spiram.c:207 passes LOW + usable - 1 to heap_caps_add_region;
    // heap_caps.c:511 reports end - start (not the physical device size).
    puts(physical == 4194304 && heap == 4194303 && external && !errors && hash == 0x678f9dc5u && pages >= 32 && !bad && internal && !exhausted ? "P4 result=pass" : "P4 result=failed");
}

// Public simulator fixture. SPDX-License-Identifier: MIT
#include <Arduino.h>
#include <esp_heap_caps.h>
#include <esp_idf_version.h>

void setup() {
  Serial.begin(115200);
  Serial.printf("P0 setup idf=%s\n", ESP.getSdkVersion());
  bool found = psramFound(); // real Arduino startup; do not replace or intercept it
  size_t total = heap_caps_get_total_size(MALLOC_CAP_SPIRAM);
  size_t free_bytes = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
  Serial.printf("P0 found=%u heap=%u free=%u\n", unsigned(found), unsigned(total), unsigned(free_bytes));
  const size_t count = 1024 * 1024;
  uint8_t *p = static_cast<uint8_t *>(heap_caps_malloc(count, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT));
  if (!p) {
    Serial.println("P0 allocation=unavailable");
    Serial.println(found ? "P0 result=detected-without-allocation" : "P0 result=no-psram");
    return;
  }
  for (size_t i = 0; i < count; ++i) p[i] = uint8_t((i * 13) ^ (i >> 15));
  uint32_t hash = 2166136261u;
  size_t errors = 0;
  for (size_t i = 0; i < count; ++i) {
    uint8_t value = p[i];
    errors += value != uint8_t((i * 13) ^ (i >> 15));
    hash = (hash ^ value) * 16777619u;
  }
  Serial.printf("P0 allocation=%u errors=%u fnv1a=%08x\n", unsigned(count), unsigned(errors), unsigned(hash));
  heap_caps_free(p);
  Serial.println("P0 result=readback-complete");
}
void loop() { delay(1000); }

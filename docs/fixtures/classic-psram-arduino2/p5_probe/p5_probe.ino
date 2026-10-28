// Public simulator fixture. SPDX-License-Identifier: MIT
#include <Arduino.h>
#include <esp_heap_caps.h>

static bool checkAllocation(unsigned pass) {
  const size_t count = 1024 * 1024;
  uint8_t *p = static_cast<uint8_t *>(heap_caps_malloc(count, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT));
  if (!p) {
    Serial.printf("P5 pass=%u allocation=unavailable\n", pass);
    return false;
  }
  volatile uint8_t *bytes = p;
  for (size_t i = 0; i < count; ++i) bytes[i] = uint8_t((i * 13) ^ (i >> 15) ^ (pass * 0x5b));
  uint32_t hash = 2166136261u;
  size_t errors = 0;
  for (size_t i = 0; i < count; ++i) {
    uint8_t value = bytes[i];
    errors += value != uint8_t((i * 13) ^ (i >> 15) ^ (pass * 0x5b));
    hash = (hash ^ value) * 16777619u;
  }
  Serial.printf("P5 pass=%u allocation=%u address=%08x errors=%u fnv1a=%08x\n",
                pass, unsigned(count), unsigned(uintptr_t(p)), unsigned(errors), unsigned(hash));
  heap_caps_free(p);
  return errors == 0;
}

void setup() {
  Serial.begin(115200);
  bool found = psramFound(); // Stock Arduino startup; no explicit init or intercept.
  size_t total = heap_caps_get_total_size(MALLOC_CAP_SPIRAM);
  size_t before = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
  Serial.printf("P5 idf=%s found=%u size=%u heap=%u free=%u\n", ESP.getSdkVersion(),
                unsigned(found), unsigned(ESP.getPsramSize()), unsigned(total), unsigned(before));
  if (!found) {
    void *p = heap_caps_malloc(1024 * 1024, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    Serial.printf("P5 absent-allocation=%s\n", p ? "unexpected" : "unavailable");
    if (p) heap_caps_free(p);
    Serial.println(!p && !total && !ESP.getPsramSize() ? "P5 result=no-psram" : "P5 result=fail");
    return;
  }
  bool first = checkAllocation(1);
  size_t after_first = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
  bool second = checkAllocation(2);
  size_t after_second = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
  Serial.printf("P5 free-before=%u after-first=%u after-second=%u\n",
                unsigned(before), unsigned(after_first), unsigned(after_second));
  Serial.println(first && second && before == after_first && before == after_second
                 ? "P5 result=pass" : "P5 result=fail");
}
void loop() { delay(1000); }

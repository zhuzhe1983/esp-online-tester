#include <Arduino.h>
#include "esp32-hal-rgb-led.h"

static constexpr uint8_t RGB_PIN = 27;
static constexpr uint8_t BRIGHTNESS = 32;

static constexpr rgb_led_color_order_t COLOR_ORDER = LED_COLOR_ORDER_RGB;

void setColor(uint8_t red, uint8_t green, uint8_t blue, const char *name) {
  rgbLedWriteOrdered(RGB_PIN, COLOR_ORDER, red, green, blue);
  Serial.printf("RGB -> %s  (R=%u, G=%u, B=%u)\n", name, red, green, blue);
}

void setup() {
  Serial.begin(115200);
  delay(1000);
  Serial.println("WS2812 RGB LED test: GPIO27");
  setColor(0, 0, 0, "OFF");
}

void loop() {
  setColor(BRIGHTNESS, 0, 0, "RED");
  delay(1000);

  setColor(0, BRIGHTNESS, 0, "GREEN");
  delay(1000);

  setColor(0, 0, BRIGHTNESS, "BLUE");
  delay(1000);

  setColor(BRIGHTNESS, BRIGHTNESS, BRIGHTNESS, "WHITE");
  delay(1000);

  setColor(0, 0, 0, "OFF");
  delay(1000);
}

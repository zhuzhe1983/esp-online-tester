#include <Arduino.h>
#include <Arduino_GFX_Library.h>
#include <esp_sleep.h>

constexpr int TFT_SCLK = 18;
constexpr int TFT_MOSI = 19;
constexpr int TFT_CS = 5;
constexpr int TFT_DC = 16;
constexpr int TFT_RST = 23;
constexpr int TFT_BL = 4;
constexpr int BUTTON_NEXT = 35;
constexpr int BUTTON_POWER = 0;
constexpr uint16_t BLACK = 0x0000;
constexpr uint16_t WHITE = 0xFFFF;
constexpr uint16_t ACCENT = 0x07FF;
constexpr uint32_t DEBOUNCE_MS = 35;

Arduino_DataBus *bus = new Arduino_ESP32SPI(TFT_DC, TFT_CS, TFT_SCLK, TFT_MOSI, GFX_NOT_DEFINED);
Arduino_GFX *display = new Arduino_ST7789(bus, TFT_RST, 1, true, 135, 240, 52, 40, 53, 40);

struct Screen {
  const char *title;
  const char *first;
  const char *second;
};
const Screen screens[] = {
  {"", "schematik", ""},
  {"", "making hardware", "less hard"},
  {"", "from idea to", "blinking LED"},
  {"", "now with 100% fewer", "magic smoke"},
  {"", "it works in", "the simulator"},
  {"processor", "Xtensa LX6", "dual core, 240 MHz"},
  {"storage", "4 MB flash", ""},
  {"analog inputs", "18 ADC channels", ""},
  {"web demo", "running in", "WebAssembly"}
};
constexpr uint8_t SCREEN_COUNT = sizeof(screens) / sizeof(screens[0]);
uint8_t screenIndex = 0;
bool previousNext = HIGH;
bool stableNext = HIGH;
uint32_t nextChangedAt = 0;
bool previousPower = HIGH;
bool stablePower = HIGH;
bool powerArmed = true;
uint32_t powerChangedAt = 0;

void centeredText(const char *text, int16_t y, uint8_t size, uint16_t color) {
  display->setTextSize(size);
  display->setTextColor(color);
  display->setCursor((display->width() - strlen(text) * 6 * size) / 2, y);
  display->print(text);
}

void drawScreen() {
  display->fillScreen(BLACK);
  display->setTextWrap(false);
  if (screenIndex == 0) {
    // One lowercase wordmark, with just its first letter accented.
    display->setTextSize(4);
    display->setCursor((display->width() - 9 * 6 * 4) / 2, (display->height() - 8 * 4) / 2);
    display->setTextColor(ACCENT);
    display->print("s");
    display->setTextColor(WHITE);
    display->print("chematik");
    return;
  }
  const Screen &screen = screens[screenIndex];
  if (screen.title[0]) {
    centeredText(screen.title, 28, 1, ACCENT);
  }
  centeredText(screen.first, 51, 2, WHITE);
  centeredText(screen.second, 75, 2, WHITE);
}

void powerOff() {
  display->fillScreen(BLACK);
  digitalWrite(TFT_BL, LOW);
  // Arm only after release, otherwise the power-off press wakes us at once.
  while (digitalRead(BUTTON_POWER) == LOW) {
    delay(10);
  }
  delay(DEBOUNCE_MS);
  esp_sleep_enable_ext0_wakeup(static_cast<gpio_num_t>(BUTTON_POWER), LOW);
  esp_deep_sleep_start();
}

void setup() {
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);
  pinMode(BUTTON_NEXT, INPUT);
  pinMode(BUTTON_POWER, INPUT_PULLUP);
  previousNext = stableNext = digitalRead(BUTTON_NEXT);
  previousPower = stablePower = digitalRead(BUTTON_POWER);
  // A wake press must be released before it can request another power-off.
  powerArmed = (stablePower == HIGH);
  display->begin();
  drawScreen();
}

void loop() {
  const uint32_t now = millis();
  const bool next = digitalRead(BUTTON_NEXT);
  if (next != previousNext) {
    nextChangedAt = now;
    previousNext = next;
  }
  if ((now - nextChangedAt) >= DEBOUNCE_MS && next != stableNext) {
    stableNext = next;
    if (stableNext == LOW) {
      screenIndex = (screenIndex + 1) % SCREEN_COUNT;
      drawScreen();
    }
  }

  const bool power = digitalRead(BUTTON_POWER);
  if (!powerArmed) {
    if (power == HIGH) {
      powerArmed = true;
      previousPower = stablePower = HIGH;
      powerChangedAt = now;
    }
  } else {
    if (power != previousPower) {
      powerChangedAt = now;
      previousPower = power;
    }
    if ((now - powerChangedAt) >= DEBOUNCE_MS && power != stablePower) {
      stablePower = power;
      if (stablePower == LOW) {
        powerOff();
      }
    }
  }
}

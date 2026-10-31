#include "esp_camera.h"
#include "mbedtls/sha256.h"
#include "esp_memory_utils.h"

const char* ssid = "YOUR_WIFI_SSID";
const char* password = "YOUR_WIFI_PASSWORD";

#define PWDN_GPIO_NUM     32
#define RESET_GPIO_NUM    -1
#define XCLK_GPIO_NUM      0
#define SIOD_GPIO_NUM     26
#define SIOC_GPIO_NUM     27
#define Y9_GPIO_NUM       35
#define Y8_GPIO_NUM       34
#define Y7_GPIO_NUM       39
#define Y6_GPIO_NUM       36
#define Y5_GPIO_NUM       21
#define Y4_GPIO_NUM       19
#define Y3_GPIO_NUM       18
#define Y2_GPIO_NUM        5
#define VSYNC_GPIO_NUM    25
#define HREF_GPIO_NUM     23
#define PCLK_GPIO_NUM     22

// Evidence probe: a common ESP32-CAM Arduino setup (camera_config_t on the stack;
// fb_location, grab_mode, sccb_i2c_port unset); frames are hashed, not served.
void setup() {
  Serial.begin(115200);
  Serial.setDebugOutput(false);
  delay(300);

  camera_config_t config;
  config.ledc_channel = LEDC_CHANNEL_0;
  config.ledc_timer = LEDC_TIMER_0;
  config.pin_d0 = Y2_GPIO_NUM;
  config.pin_d1 = Y3_GPIO_NUM;
  config.pin_d2 = Y4_GPIO_NUM;
  config.pin_d3 = Y5_GPIO_NUM;
  config.pin_d4 = Y6_GPIO_NUM;
  config.pin_d5 = Y7_GPIO_NUM;
  config.pin_d6 = Y8_GPIO_NUM;
  config.pin_d7 = Y9_GPIO_NUM;
  config.pin_xclk = XCLK_GPIO_NUM;
  config.pin_pclk = PCLK_GPIO_NUM;
  config.pin_vsync = VSYNC_GPIO_NUM;
  config.pin_href = HREF_GPIO_NUM;
  config.pin_sccb_sda = SIOD_GPIO_NUM;
  config.pin_sccb_scl = SIOC_GPIO_NUM;
  config.pin_pwdn = PWDN_GPIO_NUM;
  config.pin_reset = RESET_GPIO_NUM;
  config.xclk_freq_hz = 20000000;
  config.pixel_format = PIXFORMAT_JPEG;
  config.frame_size = FRAMESIZE_VGA;
  config.jpeg_quality = 12;
  config.fb_count = 1;

  esp_err_t err = esp_camera_init(&config);
  Serial.printf("S init=%d psram=%d\n", (int)err, (int)psramFound());
}

void loop() {
  static unsigned n = 0;
  camera_fb_t *fb = esp_camera_fb_get();
  if (!fb) { Serial.println("S no frame"); delay(500); return; }
  unsigned char hash[32];
  mbedtls_sha256(fb->buf, fb->len, hash, 0);
  Serial.printf("S frame=%u width=%u height=%u len=%u format=%d %s sha256=", n++, (unsigned)fb->width,
                (unsigned)fb->height, (unsigned)fb->len, (int)fb->format, esp_ptr_external_ram(fb->buf) ? "PSRAM" : "DRAM");
  for (int i = 0; i < 32; ++i) Serial.printf("%02x", hash[i]);
  Serial.println();
  esp_camera_fb_return(fb);
  delay(200);
}

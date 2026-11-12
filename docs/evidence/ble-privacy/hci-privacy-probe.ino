// Asks the ESP32-S3's own BLE controller, over VHCI and without a host stack, what a host sees of
// LL Privacy: the LE features, the supported-commands bits, the resolving list size, and the
// answers of the resolving-list commands, including their error cases. Prints one line per command:
// `cmd 0xOPCODE -> <event bytes in hex>`.
#include "esp_bt.h"

// Keep the controller memory: the Arduino core releases it at boot unless a sketch says it uses it.
bool btInUse() { return true; }

static volatile int got = 0;
static uint8_t evt[260];

static void on_ready() {}
static int on_packet(uint8_t *data, uint16_t n) {
  if (n > sizeof(evt)) n = sizeof(evt);
  memcpy(evt, data, n);
  got = n;
  return 0;
}
static esp_vhci_host_callback_t callbacks = {on_ready, on_packet};

static void cmd(uint16_t op, const uint8_t *p, uint8_t n) {
  uint8_t pkt[4 + 64] = {1, (uint8_t)op, (uint8_t)(op >> 8), n};
  memcpy(pkt + 4, p, n);
  got = 0;
  while (!esp_vhci_host_check_send_available()) delay(1);
  esp_vhci_host_send_packet(pkt, 4 + n);
  for (int i = 0; i < 400 && !got; i++) delay(5);
  Serial.printf("cmd 0x%04x ->", op);
  for (int i = 0; i < got; i++) Serial.printf(" %02x", evt[i]);
  Serial.println(got ? "" : " (no event)");
  delay(20);
}

// LE Add Device To Resolving List: type, address (LSB first), peer IRK, local IRK.
static void add(uint8_t last) {
  uint8_t p[39] = {0};
  p[1] = last;
  for (int i = 7; i < 39; i++) p[i] = (uint8_t)(i + last);
  cmd(0x2027, p, 39);
}
static void remove_entry(uint8_t last) {
  uint8_t p[7] = {0};
  p[1] = last;
  cmd(0x2028, p, 7);
}

void setup() {
  Serial.begin(115200);
  esp_bt_controller_config_t cfg = BT_CONTROLLER_INIT_CONFIG_DEFAULT();
  Serial.printf("init %d\n", esp_bt_controller_init(&cfg));
  Serial.printf("enable %d\n", esp_bt_controller_enable(ESP_BT_MODE_BLE));
  esp_vhci_host_register_callback(&callbacks);
}

// The probe runs when the host sends a byte, so a terminal opened after the boot sees all of it.
static void probe() {
  cmd(0x0c03, nullptr, 0);                    // Reset
  cmd(0x1001, nullptr, 0);                    // Read Local Version Information
  cmd(0x1002, nullptr, 0);                    // Read Local Supported Commands
  cmd(0x2003, nullptr, 0);                    // LE Read Local Supported Features
  cmd(0x202a, nullptr, 0);                    // LE Read Resolving List Size
  cmd(0x2029, nullptr, 0);                    // LE Clear Resolving List
  remove_entry(0);                            // remove from an empty list
  add(0);                                     // the all-zero entry Bluedroid adds
  add(0);                                     // the same entry again
  remove_entry(0);
  remove_entry(0);                            // gone already
  for (uint8_t i = 1; i <= 40; i++) {         // fill until refused
    add(i);
    if (evt[6] != 0) break;                   // the status byte of Command Complete
  }
  cmd(0x2029, nullptr, 0);
  uint8_t on = 1, two = 2;
  cmd(0x202d, &on, 1);                        // LE Set Address Resolution Enable 1
  cmd(0x202d, &two, 1);                       // an invalid value
  add(1);                                     // add while resolution is enabled, nothing running
  uint8_t off = 0;
  cmd(0x202d, &off, 1);
  uint8_t t900[2] = {0x84, 0x03}, t0[2] = {0, 0}, tmax[2] = {0x10, 0x0e}, tover[2] = {0x11, 0x0e};
  cmd(0x202e, t900, 2);                       // LE Set RPA Timeout 900 s
  cmd(0x202e, t0, 2);                         // 0 s
  cmd(0x202e, tmax, 2);                       // 3600 s
  cmd(0x202e, tover, 2);                      // 3601 s
  uint8_t pm_in[8] = {0, 1, 0, 0, 0, 0, 0, 1}, pm_out[8] = {0, 0x7f, 0, 0, 0, 0, 0, 1}, pm_bad[8] = {0, 1, 0, 0, 0, 0, 0, 2};
  cmd(0x204e, pm_in, 8);                      // LE Set Privacy Mode, entry in the list
  cmd(0x204e, pm_out, 8);                     // entry not in the list
  cmd(0x204e, pm_bad, 8);                     // invalid mode
  uint8_t rd_in[7] = {0, 1, 0, 0, 0, 0, 0}, rd_out[7] = {0, 0x7f, 0, 0, 0, 0, 0};
  cmd(0x202b, rd_in, 7);                      // LE Read Peer Resolvable Address
  cmd(0x202b, rd_out, 7);
  cmd(0x202c, rd_in, 7);                      // LE Read Local Resolvable Address
  Serial.println("done");
}

void loop() {
  if (Serial.available()) {
    while (Serial.available()) Serial.read();
    probe();
  }
  delay(10);
}

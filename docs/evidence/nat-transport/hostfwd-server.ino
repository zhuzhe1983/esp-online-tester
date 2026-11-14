// The NAT receipt's inbound probe: join the open network "esp32sim", then serve TCP port 80. Each
// connection gets one line back with its number and the bytes it sent, then the guest closes it.
#include <WiFi.h>

WiFiServer server(80);
unsigned count = 0;

void setup() {
  Serial.begin(115200);
  WiFi.mode(WIFI_STA);
  WiFi.begin("esp32sim", "");
  while (WiFi.status() != WL_CONNECTED) delay(100);
  Serial.printf("ip=%s\n", WiFi.localIP().toString().c_str());
  server.begin();
  Serial.println("listening on 80");
}

void loop() {
  WiFiClient client = server.available();
  if (!client) { delay(10); return; }
  String line;
  unsigned long t0 = millis();
  while (client.connected() && millis() - t0 < 2000) {
    if (!client.available()) { delay(1); continue; }
    char c = client.read();
    if (c == '\n') break;
    line += c;
  }
  count++;
  client.printf("conn %u got %u bytes: %s\n", count, line.length(), line.c_str());
  Serial.printf("conn %u from %s: %s\n", count, client.remoteIP().toString().c_str(), line.c_str());
  delay(10);
  client.stop();
}

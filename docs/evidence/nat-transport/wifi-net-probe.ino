// The NAT receipt's probe: scan, join the open network "esp32sim" (DHCP), a DNS lookup, SNTP from
// pool.ntp.org and an HTTP GET, each of which the user-mode NAT relays to the host's network.
#include <WiFi.h>
#include <HTTPClient.h>
#include <time.h>

const char *NET_SSID = "esp32sim";
const char *NET_PASSWORD = "";  // open network
const char *HOST = "example.org";

void printTime() {
  struct tm tm;
  if (!getLocalTime(&tm, 0)) return;
  char buf[32];
  strftime(buf, sizeof buf, "%Y-%m-%d %H:%M:%S", &tm);
  Serial.printf("time: %s UTC\n", buf);
}

void setup() {
  Serial.begin(115200);
  Serial.println("Ready");
  WiFi.mode(WIFI_STA);
  int n = WiFi.scanNetworks();
  Serial.printf("scan: %d network(s)\n", n);
  for (int i = 0; i < n; i++)
    Serial.printf("  %s rssi=%d ch=%d %s\n", WiFi.SSID(i).c_str(), WiFi.RSSI(i), WiFi.channel(i),
                  WiFi.encryptionType(i) == WIFI_AUTH_OPEN ? "open" : "secured");
  Serial.printf("connecting to %s", NET_SSID);
  WiFi.begin(NET_SSID, NET_PASSWORD);
  unsigned long t0 = millis();
  while (WiFi.status() != WL_CONNECTED && millis() - t0 < 20000) {
    delay(500);
    Serial.print('.');
  }
  Serial.println();
  if (WiFi.status() != WL_CONNECTED) {
    Serial.printf("not connected: status=%d\n", (int)WiFi.status());
    return;
  }
  Serial.printf("ip=%s gw=%s dns=%s\n", WiFi.localIP().toString().c_str(),
                WiFi.gatewayIP().toString().c_str(), WiFi.dnsIP().toString().c_str());
  IPAddress addr;
  if (WiFi.hostByName(HOST, addr))
    Serial.printf("dns: %s -> %s\n", HOST, addr.toString().c_str());
  else
    Serial.printf("dns: %s failed\n", HOST);
  configTime(0, 0, "pool.ntp.org");
  struct tm tm;
  if (getLocalTime(&tm, 15000))
    printTime();
  else
    Serial.println("time: no answer");
  HTTPClient http;
  String url = String("http://") + HOST + "/";
  http.begin(url);
  int code = http.GET();
  if (code > 0)
    Serial.printf("http GET %s -> %d, %d bytes\n", url.c_str(), code, http.getString().length());
  else
    Serial.printf("http GET %s -> %d (%s)\n", url.c_str(), code, http.errorToString(code).c_str());
  http.end();
}

void loop() {
  delay(5000);
  if (WiFi.status() == WL_CONNECTED) printTime();
}

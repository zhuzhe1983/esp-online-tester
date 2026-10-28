// Bluetooth LE central: scans for 2 s (active) every 3 s and prints each device it heard:
// `found name=<name> addr=<address> rssi=<dBm> uuid=<service or ->`, then `scan done n=<count>`.
// With `--ble` the virtual peripheral `esp32sim` (Battery Service) answers, and with `--ble-nearby`
// the devices around the station.
#include <BLEDevice.h>
#include <BLEScan.h>

void setup() {
  Serial.begin(115200);
  Serial.println("Ready");
  BLEDevice::init("scanner");
  BLEScan *scan = BLEDevice::getScan();
  scan->setActiveScan(true);
  scan->setInterval(100);
  scan->setWindow(99);
}

void loop() {
  BLEScan *scan = BLEDevice::getScan();
  BLEScanResults *found = scan->start(2, false);
  int n = found ? found->getCount() : 0;
  for (int i = 0; i < n; i++) {
    BLEAdvertisedDevice d = found->getDevice(i);
    String uuid = d.haveServiceUUID() ? String(d.getServiceUUID().toString().c_str()) : String("-");
    Serial.printf("found name=%s addr=%s rssi=%d uuid=%s\n", d.haveName() ? d.getName().c_str() : "-",
                  d.getAddress().toString().c_str(), d.getRSSI(), uuid.c_str());
  }
  Serial.printf("scan done n=%d\n", n);
  scan->clearResults();
  delay(1000);
}

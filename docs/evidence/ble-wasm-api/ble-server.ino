// The receipt's GATT server for the virtual peer: one service with one characteristic that is
// read, written and notified, as Arduino-ESP32's BLE Server example. UART0 marks advertising,
// connections and writes.
#include <BLEDevice.h>
#include <BLEServer.h>
#include <BLE2902.h>

#define SERVICE_UUID "4fafc201-1fb5-459e-8fcc-c5c9c331914b"
#define CHAR_UUID "beb5483e-36e1-4688-b7f5-ea07361b26a8"

BLECharacteristic *characteristic;
bool connected = false;
uint32_t count = 0;

class ServerCallbacks : public BLEServerCallbacks {
  void onConnect(BLEServer *) override { connected = true; Serial.println("ble: connected"); }
  void onDisconnect(BLEServer *server) override {
    connected = false;
    Serial.println("ble: disconnected");
    server->startAdvertising();
  }
};

class CharCallbacks : public BLECharacteristicCallbacks {
  void onWrite(BLECharacteristic *c) override { Serial.printf("ble: write %s\n", c->getValue().c_str()); }
};

void setup() {
  Serial.begin(115200);
  BLEDevice::init("esp32sim-server");
  BLEServer *server = BLEDevice::createServer();
  server->setCallbacks(new ServerCallbacks());
  BLEService *service = server->createService(SERVICE_UUID);
  characteristic = service->createCharacteristic(CHAR_UUID,
      BLECharacteristic::PROPERTY_READ | BLECharacteristic::PROPERTY_WRITE | BLECharacteristic::PROPERTY_NOTIFY);
  characteristic->addDescriptor(new BLE2902());
  characteristic->setCallbacks(new CharCallbacks());
  characteristic->setValue("hello");
  service->start();
  BLEAdvertising *advertising = BLEDevice::getAdvertising();
  advertising->addServiceUUID(SERVICE_UUID);
  BLEDevice::startAdvertising();
  Serial.println("ble: advertising");
}

void loop() {
  if (connected) {
    char value[16];
    snprintf(value, sizeof value, "n=%lu", (unsigned long)count++);
    characteristic->setValue(value);
    characteristic->notify();
  }
  delay(500);
}

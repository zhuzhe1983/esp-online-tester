# Air Quality v1.1 K131-V11 bring-up

This separate hardware identity depends on the original K131 digital-model implementation at 006573f0. Sources are fixed in [sources.json](evidence/airquality-v11/sources.json); manufacturer CAD and photo receipts remain under the exact v1.1 knowledge/model entries.

The current official v1.1 page links the same AirQ v1.0 carrier schematic, GDEY0154D67 datasheet and AirQUserDemo factory source as the original SKU. Explicit display routes are BUSY1/RST2/DC3/CS4/SCK5/MOSI6; buttons are A0/B8, sensors SDA11/SCL12, supply G10 and power HOLD46/WAKE42. These identical documented carrier connections permit reuse of the digital SSD1681/SCD40/SEN55 implementation. They do not establish equal whole-device behavior.

The master module changes to Stamp-S3A. Its RGB supply requires GPIO38 HIGH, while the old Stamp-S3 does not have this gate. RGB/RMT and its supply behavior are not implemented in this acceptance slice; they must not inherit an always-powered old-module LED. RF/antenna improvements also do not imply network simulation. RTC, battery, audio, external Grove, sleep and supply latch retain the original explicitly unsupported scope.

The official factory catalog points at the shared v0.2.0 complete ROM image, SHA-256 `ac47e80ba0b0e6c304e90ba10aa978f367d4369a2e449c1f4a58813a671b3216`. The source-only checkpoint does not claim the v1.1 runtime has passed; actual board selection, framebuffer, controls and browser verification follow in the implementation commit.

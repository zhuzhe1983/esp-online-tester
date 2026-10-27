# Air Quality K131 bring-up

Source checkpoint for original K131. K131-V11 keeps its separate identity: newer Stamp-S3A adds a GPIO38 RGB power switch; the published external display/sensor/control routes match, but this does not establish identical firmware acceptance. Sources and retrieval hashes are in [sources.json](evidence/airquality/sources.json). No runtime capability is enabled at this checkpoint.

The official factory image v0.2.0 is 8,323,072 bytes, SHA-256 `ac47e80ba0b0e6c304e90ba10aa978f367d4369a2e449c1f4a58813a671b3216`, and identifies IDF v4.4.6-dirty. Its bytes remain an ignored remote input. The public AirQUserDemo repository supplies initialization and button behavior, but current HEAD is not asserted to be the exact binary build.

| Circuit | Official source route |
| --- | --- |
| GDEY0154D67 / SSD1681 | SCK5 MOSI6 CS4 DC3 RESET2 BUSY1; 200×200 monochrome |
| Buttons | A=G0, B=G8; WAKE G42 and HOLD G46 belong to power control |
| I²C | SDA11/SCL12: SCD40 0x62, SEN55 0x69, RTC8563 0x51 |
| SEN55 supply | G10: source sets LOW to enable; inspect schematic polarity separately from name |
| Battery/buzzer | G14 ADC battery divider / G9 buzzer |
| Grove | separate SDA13/SCL15; no visual connector placement inferred from I²C routing |

The display datasheet's 0x44 X address uses byte units and two parameter bytes, unlike SSD1677 pixel-address words. It therefore requires a separately identified SSD1681 model. The PDF contains apparent copied default-window values (0x15,0x127), while this glass and initialization explicitly use 200×200; retain that discrepancy rather than silently presenting all register defaults as silicon verified. BUSY polarity also conflicts between a prose paragraph and the command table/official driver; the command table and driver use active HIGH.

The factory program initializes its filesystem, display and Wi-Fi before I²C; it requires SCD40 ready polling before installing button callbacks. Missing sensors cannot be replaced by mocked application output. Implement source-backed digital sensor commands with CRC, readiness and explicit virtual sample values; analog environmental dynamics and network/cloud access remain separate unimplemented scope.

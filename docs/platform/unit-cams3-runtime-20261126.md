# Unit CamS3 digital camera capture

| Product | Board | Sensor / SCCB | Memory |
| --- | --- | --- | --- |
| U174 Unit CamS3 | `unit-cams3` | OV2640, 0x30; 8-bit banked registers | 16 MiB Flash / 8 MiB PSRAM |
| U174-B Unit CamS3-5MP | `unit-cams3-5mp` | PY260 / MEGA_CCM, 0x1f; ID 0x039e; 16-bit controls | 16 MiB Flash / 8 MiB PSRAM |

Both boards use SCCB SDA17/SCL41, active-low RESET21, grounded PWDN,
XCLK11 and D0..7 = 6/15/16/7/5/10/4/13, PCLK12, HREF18, VSYNC42.
GPIO0 is a download-header strap, not an added physical button. GPIO14 sinks
the blue D1 LED. The common SD socket uses SPI CLK39/MOSI38/MISO40/CS9.

The guest configures the sensor through either I2C controller with the physical
SCCB matrix route, or through open-drain GPIO transactions. The board's fixed
sensor remains shared across controllers. Incorrect pins/address or asserted
reset do not ACK. Reset clears geometry and output configuration. MCU reboot
also resets sensor configuration, while retaining the separately supplied host
picture/JPEG and SD bytes. A fresh platform instance has fresh sensor state.

LCD_CAM receives bytes through GDMA trigger 5 descriptors, including owner
checks, descriptor lengths, EOF and error interrupts. DVP matrix inputs and
XCLK routing are checked before delivery. XCLK can come from the programmed
LCD_CAM clock and divider, or a routed LEDC output. Wrong/missing data, HREF,
PCLK, VSYNC or XCLK routing, a stopped sensor, or reset cannot deliver bytes
from an earlier cached frame. Data/PCLK/HREF inversion, 16-bit DVP and VH/DE
capture remain outside the implemented byte-stream subset. VSYNC inversion is
accepted as capture-edge selection.

An explicit RGB888 picture supplies RGB565 or YUYV conversion at the sensor's
configured dimensions. The existing host-source default is a black virtual
picture, not physical optical capture. JPEG mode accepts only an already
encoded host baseline JPEG of exactly the selected size and delivers it
verbatim. There is no JPEG encoder, decoder or raw fallback in JPEG mode.
PY260 uses its own published MEGA_CCM controls (0x0120/0x0121), not OV5640 or
GC0308 registers. The M5 legacy formats 0x10/0x11 and upstream v2.1.4 formats
2/3 select RGB565/YUYV respectively; 1 selects JPEG. Published resolution codes
1..9 cover QVGA, VGA, HD, UXGA, FHD, 5MP, 96x96, 128x128 and 320x320.

The shared OV3660 model is separate: ID 0x3660 at 0x300a/b, reset/standby
0x3008, geometry 0x3808..b, JPEG bit 0x3821 and output format 0x4300.
Its inclusive array window uses the public driver's 2080x1548 full window;
this model is available to the classic-camera board lane.

Sources are retained with exact decoded-source hashes, retrieval UTC and pinned
references in [the source manifest](research/m5-cams3-runtime-20261126/sources.json).
M5's OV2640 factory pin header omits RESET (`-1`), while current product pin
documentation and the common schematic connect GPIO21. The model follows the
physical connection and still permits firmware that leaves RESET untouched.
The current 5MP branch has a legacy one-byte ID probe, supported alongside its
16-bit control pointer. The protocol's firmware-version byte is unknown (0),
and unmodeled ISP controls retain writes without inventing optical effects.

Validation uses original LX7 S32I/L32I guest diagnostics in
`esp32s3/tests/unit_cams3.rs`: real SCCB transactions, explicit two-color source
bytes into DMA RAM, JPEG marker/byte delivery, clock and route rejection,
descriptor ownership, reset/reboot, active-low LED and SPI SD sector roundtrips.
These diagnostics do not establish original factory image compatibility.

Timing retains the existing approximate 10 Hz frame schedule, 5% blanking and
half-period active transfer; it is not sensor PLL/PCLK timing calibration.
Reset RC, power rail ramps and PY260 startup wait are not modeled. A floating
RESET is deasserted by ideal model policy. No physical hardware, optics,
exposure, focus, ISP, PDM microphone, USB UVC or radio/cloud service validation
is claimed. Manufacturer factory-source records remain separate and unchanged.

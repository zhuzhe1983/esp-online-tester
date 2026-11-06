# RAM-only fixture for the unchanged official UIFlow CoreMatrix image.
# Host: U001-D ENV-IV at 22 C / 60% RH / 990 hPa and U136 at 1000 lx.
import M5
import time
import struct
from machine import I2C, Pin
from driver.m5ioe1 import M5IOE1

M5.begin()
bus = I2C(0, sda=Pin(0), scl=Pin(1), freq=400000)
ioe = M5IOE1(bus, addr=0x4F)
ioe.set_pin_mode(5, 1)
ioe.set_pin_value(5, True)
time.sleep_ms(10)

def nack(call):
    try:
        call()
    except OSError:
        return
    raise AssertionError("unwired or unpowered sensor acknowledged")

def crc(data):
    value = 255
    for byte in data:
        value ^= byte
        for bit in range(8):
            value = ((value << 1) ^ (0x31 if value & 0x80 else 0)) & 255
    return value

assert bus.readfrom_mem(0x76, 0xD0, 1) == b"\x58"
bus.writeto(0x44, b"\xfd")
time.sleep_ms(10)
sample = bus.readfrom(0x44, 6)
assert sample[2] == crc(sample[:2]) and sample[5] == crc(sample[3:5])
t = -45 + 175 * int.from_bytes(sample[:2], "big") / 65535
rh = -6 + 125 * int.from_bytes(sample[3:5], "big") / 65535
assert abs(t - 22) < 0.01 and abs(rh - 60) < 0.01, (t, rh)
# Bosch BMP280 datasheet rev1.26 compensation from guest calibration/raw bytes.
cal = struct.unpack("<HhhHhhhhhhhh", bus.readfrom_mem(0x76, 0x88, 24))
bus.writeto_mem(0x76, 0xF4, b"\x25")
time.sleep_ms(10)
data = bus.readfrom_mem(0x76, 0xF7, 6)
adc_p = int.from_bytes(data[:3], "big") >> 4
adc_t = int.from_bytes(data[3:], "big") >> 4
fine = (adc_t / 16384 - cal[0] / 1024) * cal[1] + (adc_t / 131072 - cal[0] / 8192) ** 2 * cal[2]
v1 = fine / 2 - 64000
v2 = v1 * v1 * cal[8] / 32768 + v1 * cal[7] * 2
v2 = v2 / 4 + cal[6] * 65536
v1 = (cal[5] * v1 * v1 / 524288 + cal[4] * v1) / 524288
v1 = (1 + v1 / 32768) * cal[3]
pressure = (1048576 - adc_p - v2 / 4096) * 6250 / v1
pressure = (pressure + (cal[11] * pressure * pressure / 2147483648 + pressure * cal[10] / 32768 + cal[9]) / 16) / 100
assert abs(pressure - 990) < 0.02, pressure
bus.writeto(0x23, b"\x20")
time.sleep_ms(200)
raw = int.from_bytes(bus.readfrom(0x23, 2), "big")
assert raw == 1200, raw
print("CM_GROVE_SAMPLE", sample, t, rh, pressure, raw)
nack(lambda: bus.readfrom_mem(0x45, 0, 1))
wrong = I2C(0, sda=Pin(3), scl=Pin(4), freq=400000)
nack(lambda: wrong.readfrom_mem(0x76, 0xD0, 1))
bus = I2C(0, sda=Pin(0), scl=Pin(1), freq=400000)
assert bus.readfrom_mem(0x76, 0xD0, 1) == b"\x58"
print("CM_GROVE_WRONG_ADDRESS_PINS_NACK")
ioe.set_pin_value(5, False)
nack(lambda: bus.readfrom_mem(0x76, 0xD0, 1))
nack(lambda: bus.readfrom(0x23, 2))
assert len(bus.readfrom_mem(0x6E, 0, 1)) == 1
ioe.set_pin_value(5, True)
time.sleep_ms(10)
assert bus.readfrom(0x23, 2) == b"\x00\x00"
assert bus.readfrom_mem(0x76, 0xD0, 1) == b"\x58"
print("CM_GROVE_POWER_RESET_PASS")

# Original UIFlow 2.5.4 RAM-only raw REPL fixture. No replacement boot image.
# Bosch SensorAPI 41129fc, standard BMI270 any-motion feature page 1:
# duration=5 * 20 ms; XYZ enabled; 83 mg; out_conf=7 -> INT_STATUS_0 bit6.
# M5PM1 manual v1.9: GPIO4 falling wake, LDO hold is configured by the caller.
import M5
import time
from machine import I2C, Pin

M5.begin()
p = I2C(0, sda=Pin(0), scl=Pin(1), freq=400000)
print("CM_IMU_INIT", p.readfrom_mem(0x68, 0x21, 1)[0])
p.writeto_mem(0x68, 0x7D, b"\x04")  # accelerometer power
p.writeto_mem(0x68, 0x2F, b"\x01")  # feature page 1
p.writeto_mem(0x68, 0x3C, b"\x05\xe0\xaa\xb8")
p.writeto_mem(0x68, 0x58, b"\x00")  # unmap M5.begin() data-ready IRQs
p.writeto_mem(0x68, 0x53, b"\x08")  # INT1 active-low push-pull output
p.writeto_mem(0x68, 0x55, b"\x01")  # latched interrupt
p.writeto_mem(0x68, 0x56, b"\x40")  # any-motion maps to INT1
p.writeto_mem(0x6E, 0x18, b"\x10\x00")  # GPIO4 falling edge wake
p.writeto_mem(0x6E, 0x05, b"\x00")  # clear prior PM1 wake cause
time.sleep_ms(50)
assert p.readfrom_mem(0x6E, 0x12, 1)[0] & 0x10  # INT1 idle high before sleep
print("CM_MOTION_CONFIG", list(p.readfrom_mem(0x68, 0x3C, 4)), list(p.readfrom_mem(0x68, 0x53, 6)), list(p.readfrom_mem(0x6E, 0x10, 10)))
print("CM_MOTION_READY")

# Internal guest program for the original CoreMatrix UIFlow2 firmware.
# This source is uploaded to RAM through MicroPython raw REPL, never substituted
# for a manufacturer's firmware or installed as boot.py/main.py.
import M5
import time
from machine import I2C, Pin, SDCard
from driver.m5ioe1 import M5IOE1

M5.begin()
M5.Display.setRotation(0)
_cm_bus = I2C(0, sda=Pin(0), scl=Pin(1), freq=400000)
_cm_ioe = M5IOE1(_cm_bus, addr=0x4F)
_cm_sd = None


def cm_matrix(points=None, brightness=255):
    M5.Display.setBrightness(brightness)
    M5.Display.clear(0x000000)
    if points is None:
        M5.Display.clear(0xFFFFFF)
    else:
        for x, y in points:
            M5.Display.drawPixel(x, y, 0xFFFFFF)
    print("CM_MATRIX", brightness)


def cm_brightness(value):
    M5.Display.setBrightness(value)
    print("CM_BRIGHTNESS", value)


def cm_led_power(enabled):
    _cm_ioe.set_pin_mode(4, 1)
    _cm_ioe.set_pin_value(4, bool(enabled))
    print("CM_LED_POWER", int(bool(enabled)))


def cm_reinitialize_matrix():
    # The same command sequence as the pinned official M5GFX TM1680 driver.
    _cm_bus.writeto(0x72, b"\x80\xa4\x98\x81\xbf\x83")
    print("CM_MATRIX_INIT")


def cm_buttons():
    M5.update()
    time.sleep_ms(40)
    M5.update()
    print("CM_BUTTONS", int(M5.BtnA.isPressed()), int(M5.BtnB.isPressed()), int(M5.BtnC.isPressed()))


def cm_imu():
    M5.update()
    import json
    print("CM_IMU_JSON", json.dumps({"enabled": M5.Imu.isEnabled(), "accel": M5.Imu.getAccel(), "gyro": M5.Imu.getGyro(), "raw": list(_cm_bus.readfrom_mem(0x68, 0x0C, 6)), "range": _cm_bus.readfrom_mem(0x68, 0x41, 1)[0] & 3}))


def cm_power():
    print("CM_POWER", M5.Power.getBatteryVoltage(), M5.Power.getVBUSVoltage(), M5.Power.getExtOutput())
    print("CM_RAILS", _cm_ioe.read_reg16(5))


def cm_buzzer(hz=1200, duration_ms=250):
    M5.Speaker.begin()
    M5.Speaker.setVolume(128)
    M5.Speaker.tone(hz, duration_ms)
    print("CM_BUZZER", hz, duration_ms)


def cm_buzzer_pwm(hz=1200):
    # Stop the official speaker task before testing independent expander PWM.
    # Otherwise its idle cleanup may overwrite the manually configured output.
    M5.Speaker.end()
    # The product's IO11 is M5IOE1 PWM channel 2, not an ESP GPIO.
    _cm_ioe.set_pin_mode(2, 1)
    _cm_ioe.set_pin_drive(2, 0)
    _cm_ioe.set_pin_value(2, True)
    _cm_ioe.set_pin_mode(11, 1)
    _cm_ioe.set_pwm_frequency(hz)
    _cm_ioe.set_pwm_duty_u12(2, 2048)
    print("CM_PWM", _cm_ioe.read_reg16(0x25), _cm_ioe.read_reg16(0x1F))


def cm_buzzer_power(enabled):
    _cm_ioe.set_pin_mode(2, 1)
    _cm_ioe.set_pin_drive(2, 0)
    _cm_ioe.set_pin_value(2, bool(enabled))
    level = _cm_ioe.read_reg16(5) & 2
    assert bool(level) == bool(enabled), (_cm_ioe.read_reg16(3), _cm_ioe.read_reg16(5), _cm_ioe.read_reg16(0x13))
    print("CM_BUZZER_POWER", int(bool(enabled)))


def cm_sd(write=True):
    # This writes only a deliberately isolated test card owned by the probe.
    import os
    global _cm_sd
    _cm_ioe.set_pin_mode(1, 1)
    _cm_ioe.set_pin_value(1, True)
    if _cm_sd is None:
        _cm_sd = SDCard(slot=2, sck=Pin(25), miso=Pin(26), mosi=Pin(27), cs=Pin(28), freq=1000000)
        os.mount(_cm_sd, "/sd")
    if write:
        with open("/sd/corematrix-fixture.txt", "w") as f:
            f.write("COREMATRIX_SD_PERSISTENCE_1\n")
        os.sync()
    with open("/sd/corematrix-fixture.txt") as f:
        assert f.read() == "COREMATRIX_SD_PERSISTENCE_1\n"
    print("CM_SD", "WRITE_READ" if write else "PERSISTED_READ")


def cm_wrong_address():
    try:
        _cm_bus.writeto(0x73, b"\x81\x83")
    except OSError:
        print("CM_WRONG_ADDRESS_NACK")
        return
    raise AssertionError("unexpected TM1680 response at 0x73")


print("CM_READY", M5.getBoard(), M5.Display.width(), M5.Display.height())

# Run only in an isolated copy of the official firmware flash.
import machine
import os
import time

data = bytes(range(256)) * 64 + b'CoreMatrix internal Flash retention\n'
path = '/flash/corematrix-system-audit.bin'
with open(path + '.tmp', 'wb') as stream:
    assert stream.write(data) == len(data)
    stream.flush()
os.rename(path + '.tmp', path)
os.sync()
with open(path, 'rb') as stream:
    assert stream.read() == data
print('CM_SYSTEM_FLASH_WRITTEN', len(data), sum(data), os.stat(path)[6])

rtc = machine.RTC()
rtc.datetime((2026, 11, 6, 4, 12, 34, 10, 0))
start_ticks = time.ticks_ms()
start_rtc = rtc.datetime()
time.sleep_ms(1250)
elapsed = time.ticks_diff(time.ticks_ms(), start_ticks)
end_rtc = rtc.datetime()
assert 1250 <= elapsed < 1500, elapsed
assert end_rtc[:3] == (2026, 11, 6), end_rtc
assert 1 <= end_rtc[6] - start_rtc[6] <= 2, (start_rtc, end_rtc)
print('CM_SYSTEM_RTC_ACTIVE', start_rtc, end_rtc, elapsed)

bus = machine.I2C(0, sda=machine.Pin(0), scl=machine.Pin(1), freq=100000)
addresses = bus.scan()
assert addresses == [0x4f, 0x68, 0x6e, 0x72], addresses
print('CM_SYSTEM_I2C', addresses)
print('CM_SYSTEM_RESET_REQUEST')
machine.reset()

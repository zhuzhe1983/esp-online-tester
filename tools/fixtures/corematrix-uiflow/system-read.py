import machine
import os
import time

data = bytes(range(256)) * 64 + b'CoreMatrix internal Flash retention\n'
path = '/flash/corematrix-system-audit.bin'
with open(path, 'rb') as stream:
    actual = stream.read()
assert actual == data, (len(actual), sum(actual))
try:
    os.stat(path + '.tmp')
    raise AssertionError('rename left the temporary file')
except OSError as error:
    assert error.args[0] == 2, error
print('CM_SYSTEM_FLASH_RETAINED', len(actual), sum(actual), os.stat(path)[6])
retained_rtc = machine.RTC().datetime()
assert retained_rtc[:3] == (2026, 11, 6), retained_rtc
assert retained_rtc[4:6] == (12, 34) and 11 <= retained_rtc[6] < 60, retained_rtc
print('CM_SYSTEM_RTC_AFTER_RESET', retained_rtc)
assert machine.reset_cause() == machine.HARD_RESET
print('CM_SYSTEM_RESET_CAUSE', machine.reset_cause())
os.remove(path)
os.sync()
print('CM_SYSTEM_DONE')

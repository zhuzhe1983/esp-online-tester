# Original MicroPython BLE host/controller; callbacks come from RX DMA and IRQ.
import bluetooth, time, ujson
b = bluetooth.BLE()
b.active(True)
cm_ble_scans = []
cm_ble_done = False
def cm_ble_irq(event, data):
    global cm_ble_done
    if event == 5:
        kind, addr, adv_type, rssi, payload = data
        cm_ble_scans.append([kind, bytes(addr).hex(), adv_type, rssi, bytes(payload).hex()])
    elif event == 6:
        cm_ble_done = True
def cm_ble_scan():
    global cm_ble_done
    cm_ble_scans.clear()
    cm_ble_done = False
    b.irq(cm_ble_irq)
    b.gap_scan(300, 30000, 30000, False)
    time.sleep_ms(500)
    assert cm_ble_done
cm_ble_scan()
print("CM_BLE_SCAN_JSON", ujson.dumps(cm_ble_scans))
b.gap_advertise(100000, b"\x02\x01\x06\x04\x09C61")
time.sleep_ms(250)
b.gap_advertise(None)
print("CM_BLE_ADV_DONE")

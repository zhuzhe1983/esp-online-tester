# ESP-Hosted: an ESP32-P4 with an ESP32-C6 co-processor

Two firmware images that give an ESP32-P4 WiFi through an ESP32-C6 on SDIO, as on the Waveshare
ESP32-P4-WIFI6 and Espressif's P4 boards:

- `station/`: ESP-IDF v5.5.3's `wifi/getting_started/station` example (Unlicense or CC0-1.0, as
  its sources say) for the ESP32-P4 rev < 3, unchanged except for `sdkconfig.defaults`, which adds
  the target, the revision and the virtual access point's SSID `esp32sim` and password
  `esp32sim-pass`, and `main/idf_component.yml`, which pins espressif/esp_hosted to `==2.12.13`
  so the host matches the slave exactly. `dependencies.lock` pins the rest: esp_wifi_remote 1.6.5,
  eppp_link 1.1.6, esp_serial_slave_link 1.1.2, wifi_remote_over_eppp 0.3.3. Its ESP-Hosted
  defaults for the P4 are the reference board's: SDIO slot 1 through the GPIO matrix, CLK 18,
  CMD 19, D0-D3 14-17, four lines at 40 MHz, streaming receive, the C6's reset on GPIO 54
  (active high: low resets).
- `slave/`: ESP-Hosted 2.12.13's `slave` example (Apache-2.0), unchanged, for the ESP32-C6:
  `idf.py create-project-from-example "espressif/esp_hosted==2.12.13:slave"`, then
  `idf.py set-target esp32c6 build`. SDIO is the default transport on the C6 (CMD 18, CLK 19,
  D0-D3 20-23, streaming send, high speed). Only its `dependencies.lock` is kept here.

Built with ESP-IDF v5.5.3: `idf.py set-target esp32p4 build` in `station/`, with
`sdkconfig.defaults` selecting rev < 3. `station/sdkconfig.defaults.rev3` is the same without the
revision lines, for rev ≥ 3 (IDF v5.5.4, `idf.py -D SDKCONFIG_DEFAULTS=sdkconfig.defaults.rev3
set-target esp32p4 build`): `p4-hosted-rev3-bootloader.bin` and `p4-hosted-rev3-station.bin` (the
partition table is the same), the input of the golden `hosted-p4-rev3` and the browser demo
`p4-c6-hosted`. The images are `web/wasm/fw/public/p4-hosted-*.bin` and
`c6-hosted-*.bin`, pinned and fetched by `tools/fetch-p4-firmware.sh` (licences in
`hosted-NOTICE.txt`, committed), and are the input of the `hosted_p4` golden:

    ../../target/release/esp32sim-p4 --flash-mb 2 \
      --bootloader station/build/bootloader/bootloader.bin \
      --ptable station/build/partition_table/partition-table.bin --app station/build/wifi_station.bin \
      --coproc c6:bootloader=slave/build/bootloader/bootloader.bin,ptable=slave/build/partition_table/partition-table.bin,app=slave/build/network_adapter.bin,otadata=slave/build/ota_data_initial.bin,stub=bb_init=0,elf=slave/build/network_adapter.elf \
      --wifi ssid=esp32sim,psk=esp32sim-pass --net none --max-seconds 4

The P4 resets the C6 on GPIO 54, waits 1.5 s, brings the SDIO card up (CMD52 I/O reset, CMD0,
CMD8, CMD5, CMD3, CMD7, the CCCR and the function's block size), opens the data path with a
slave interrupt, receives the slave's INIT event (chip esp32c6, capabilities, its firmware
version, which matches the host's and so prints no "Version mismatch"), sends the WiFi RPCs, and
takes a DHCP lease through the C6: `wifi station: got ip:10.0.2.15` at 3.77 s. The C6's console
follows on stdout with `[c6] ` in front; see ../../docs/esp32p4.md, "The co-processor".

`stub=bb_init=0` skips the C6 PHY's baseband calibration, as for every C6 WiFi run here
(../../docs/esp32c6.md); the golden gives its address, `0x420bac5c`, since it ships no ELF.

## Raw throughput

ESP-Hosted's own transport test (RawTP) sends 1460-byte packets as fast as the link takes them
and reports the rate every second on both consoles. The images `p4-hosted-rawtp-rx.bin`,
`p4-hosted-rawtp-tx.bin` and `c6-hosted-rawtp-slave.bin` are the two examples above built with it
on, nothing else changed; they boot with the bootloaders, partition tables and OTA data above.
For the host, these lines added to `station/sdkconfig.defaults`:

    CONFIG_ESP_HOSTED_RAW_THROUGHPUT_TRANSPORT=y
    CONFIG_ESP_HOSTED_RAW_THROUGHPUT_RX_FROM_SLAVE=y     # p4-hosted-rawtp-rx: the C6 sends
    CONFIG_ESP_HOSTED_RAW_TP_REPORT_INTERVAL=1

with `CONFIG_ESP_HOSTED_RAW_THROUGHPUT_TX_TO_SLAVE=y` in place of `..._RX_FROM_SLAVE` for
`p4-hosted-rawtp-tx` (the P4 sends); for the slave, these added to its `sdkconfig.defaults`:

    CONFIG_ESP_RAW_THROUGHPUT_TRANSPORT=y
    CONFIG_ESP_RAW_TP_REPORT_INTERVAL=1

Both sides wait 5 s after the data path opens before they send (`sleep(5)` in their `stats.c`),
so traffic starts near 7.7 s. The goldens `hosted_rawtp_rx` and `hosted_rawtp_tx` run each pair
for 10 s with the SD bus timed (`--sd-timing`); the slave's `bb_init` is at `0x420bb020` in this
build:

    ../../target/release/esp32sim-p4 --flash-mb 2 --bootloader $FW/p4-hosted-bootloader.bin \
      --ptable $FW/p4-hosted-ptable.bin --app $FW/p4-hosted-rawtp-rx.bin --sd-timing \
      --coproc c6:bootloader=$FW/c6-hosted-bootloader.bin,ptable=$FW/c6-hosted-ptable.bin,app=$FW/c6-hosted-rawtp-slave.bin,otadata=$FW/c6-hosted-otadata.bin,stub=0x420bb020=0 \
      --wifi ssid=esp32sim,psk=esp32sim-pass --net none --max-seconds 10

# M5 wired Ethernet digital subset

The shared GMAC implements MDIO and enhanced chained/ring descriptors using the
ESP-IDF v5.4.2 `emac_mac_struct.h`, `emac_dma_struct.h` and `emac_hal.h` fields.
Source fetch receipts, content hashes and original Apache-2.0 headers are retained
in `evidence/m5-ethernet-runtime-20261126/sources.json`.

The digital IP101G peer has address1 and PHY IDs0243/0c54. Its page16 CSSR30 reports
link bit8 and operation6 for a configured 100 Mbps full duplex digital peer; PCR20
and page1 PSCR17 are separate from page16 ISR17. Identity reads require the actual
MDC/MDIO routes, PHY reset released and the MAC clock. RMII data needs all six
native pad functions, reference input and MAC/interface clocks. The external PHY
oscillator is an ideal 50 MHz source, not an analog PoE or oscillator simulation.
MDIO completes after32 virtual microseconds; reset/autonegotiation are simplified.

TX reads guest descriptors and payload from the DMA window. RX writes Ethernet
bytes plus a calculated FCS, frame length and ownership back to guest RAM. Invalid
memory, owner loss, broken routes and bounded/cyclic chains cannot manufacture a
transmitted frame. Interrupt status is W1C. The register struct fields place FBE13,
AIS15 and NIS16; v5.4.2's legacy LL interrupt-mask defines differ after bit10, so the
struct fields are authoritative here. Checksum insertion, PTP, collision/retry,
PHY interrupt pin timing and TCP offload are outside this subset.

The default network is the existing local VirtualNet (ARP/DHCP/ICMP etc.). An
explicit host relay exchanges bounded raw Ethernet frames through the existing
WASM Ethernet ABI. Wired endpoints do not require a fictitious Wi-Fi association.
Guest resets clear queues/flows while retaining the host relay selection. Real
remote connectivity still requires an explicit host/helper transport.

The classic U138 adapter follows its IP101G schematic: MDC23, MDIO18,
TX_EN21/TXD0=19/TXD1=22/RXD0=25/RXD1=26/CRS_DV27, REFCLK0 and active-low reset5.
ESP32-P4 wiring and clocks are independently adapted for U213. No factory
firmware compatibility or physical hardware validation follows from these tests.

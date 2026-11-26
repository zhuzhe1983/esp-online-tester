# Unit PoE-P4 U213 digital runtime

`m5stack-poe-p4` is an independent ESP32-P4 board, 16 MiB Flash and 32 MiB
PSRAM, using a rev < 3 virtual ABI. Its IP101GRI wiring and PHY address are
fixed by the official Unit PoE-P4 documentation and UserDemo
`2840dceb6d3c6d9e51b82fff3df4bb1bf22d463c`.
The pinned `sdkconfig` selects IP101, address 1, MDC31, MDIO52 and reset51;
the IDF P4 default configuration and product table agree on the RMII pads.
[Manufacturer source receipts](evidence/m5stack-poep4-runtime-20261126/source-receipts.json)
retain the fetched source bytes' identities. The shared
[Ethernet model and primary headers](m5-ethernet-runtime-20261126.md) document
its digital protocol scope separately from analog Ethernet/PoE behavior.

| Signal | Physical pad / protocol |
| --- | --- |
| SMI | MDC GPIO31 -> matrix output108; MDIO GPIO52 -> output109 and input107 |
| RMII TX | TX_EN49, TXD0=34, TXD1=35; native mux function3 or exact matrix routes |
| RMII RX | RXD0=29, RXD1=30, CRS_DV28; native input function3 or exact input routes |
| Reference clock | GPIO50, native input function3; ideal external 50 MHz |
| PHY reset | GPIO51 active low; independent of MAC register reset |
| User key | GPIO45 active low |
| RGB | GPIO17 red, GPIO15 green, GPIO16 blue; common anode, inverted high-time fraction |

The P4 adapter mounts the shared GMAC MAC/DMA registers at 0x50098000 and
0x50099000. It checks HP_SYS_CLKRST SYSCLK13, RMII/RX/TX clocks, HP_SYSTEM
GMAC_CTRL0 PHY_INTF_SEL at +0x14C bits4:2 and LP_CLKRST pad clock/reset.
EMAC DMA reads and writes only L2/PSRAM through the same page-bounded MMU
view used by the other P4 DMA engines. It cannot invoke MMIO side effects
or substitute host bytes for unmapped guest buffers. P4 currently has no
translated decode cache requiring page invalidation.

The default digital network peer is VirtualNet. The existing bounded host
Ethernet relay ABI is also connected directly to this wired endpoint. It
does not use Wi-Fi association or a C6 coprocessor. MCU reset discards MAC
DMA state and queued host frames, while preserving the host's relay choice
and independently powered PHY register state. Physical reset51 or a PHY
BMCR reset resets the PHY. MAC reset alone cannot clear PHY power-down.

`esp32p4/tests/poe_p4_ethernet.rs` executes original RV32 instructions for
SMI polling and enhanced TX/RX descriptors, with descriptors in L2 and
Ethernet bytes in guest-mapped PSRAM. The probes require actual raw host
frames, RX ownership/length/FCS writes, W1C status and guest restart.
Wrong PHY address, separate MDI input route, reference pad, PHY reset,
disabled clock and unmapped PSRAM cannot publish a frame. The separate
`poe_p4.rs` guest verifies key45 and common-anode RGB, wrong GPIO18 and
reset removing the old LED drive.

```sh
cargo test -p esp32p4 --test poe_p4 --test poe_p4_ethernet
```

`node tools/probe-poep4.mjs` executes a deterministic 4,512-byte original RV32
SRAM application through the actual platform API, Worker and WASM. The guest
checks wrong PHY/MDI routing and a disconnected reference clock, transmits
60 actual PSRAM bytes, validates all words of a distinct host RX frame and
the enhanced descriptor's FCS-inclusive status, then reads USER45 to control
common-anode GPIO17. It passes pause and replacement-machine reset with a
second exact bidirectional exchange. A real UART0 `G` byte coordinates host
raw transport selection before transmission after the platform's automatic
reset/start. It does not fabricate a queued Ethernet frame. The
[Worker020 receipt](evidence/m5stack-p4-worker-20261126/poep4-worker020.json)
records the executed binary and harness hashes with WASM
`0cc18d96042c0b2707afbe3b80102cabcbdd6868fccfcc84504cabed8b3179a2`.
Local raw Ethernet ABI acceptance does not establish a public relay service
or Internet reachability.

This is partial digital support, not physical Ethernet/PoE acceptance or
official UserDemo Web application validation. PoE negotiation, electrical
isolation, power/current, IR, USB applications and external MIPI display or
camera accessories remain unverified or unimplemented. U213 has no
integrated screen, and attaching its board does not manufacture one.

# ESP32-P4 rev < 3 against rev >= 3: the registers of the modelled blocks

What changes between ESP-IDF v5.5.4's `components/soc/esp32p4/register/hw_ver1` (rev < 3) and
`hw_ver3` (rev >= 3) in the register headers of the blocks `esp32p4` implements, keyed by
register offset and compared by field layout (bit position and width), names ignored: several
headers rename registers and fields without moving them (USB-Serial/JTAG, HP_SYSTEM's `SYS_`
prefix, the interrupt matrix's `CORE0_` prefix), which a name-keyed comparison counts as removed
and added.

    cd components/soc/esp32p4/register     # ESP-IDF v5.5.4
    python3 offdiff.py hw_ver1/soc hw_ver3/soc assist_debug_reg.h cache_reg.h sdmmc_reg.h \
      spi_mem_c_reg.h spi1_mem_c_reg.h spi_mem_s_reg.h spi1_mem_s_reg.h sha_reg.h \
      usb_serial_jtag_reg.h interrupt_core0_reg.h gpio_reg.h systimer_reg.h hp_system_reg.h \
      hp_sys_clkrst_reg.h lp_system_reg.h lp_clkrst_reg.h lp_timer_reg.h i2c_ana_mst_reg.h \
      efuse_reg.h > offdiff.txt

`uart_reg.h` and `timer_group_reg.h` define their registers through indexed macros the script
does not parse; their `#define` lines are identical between the two directories apart from one
line's whitespace. The USB-OTG core and the CLIC have no header in these directories.

Result ([offdiff.txt](offdiff.txt)):

- No register of these blocks moves within its header. A register that moves to another header is
  out of this comparison's sight; see "Between headers" below.
- Unchanged: ASSIST_DEBUG, SDMMC, SHA, USB-Serial/JTAG, LP_CLKRST, LP_TIMER, I2C_ANA_MST (and UART,
  TIMG).
- Bits or registers added only: CACHE (a preload bit 7), the interrupt matrix (bits 6 and 7 of every
  map register, `SRC_PASS_IN_SEC` and `SRC_IN_SEC_FLAG` for the TEE, and new sources and status
  registers at the end), HP_SYS_CLKRST, HP_SYSTEM, SPI_MEM_C/S (interrupt bits 26-27, PMS attribute
  bits, new registers), SPI1_MEM_C/S `CTRL1` (bits 12-20 and 23), GPIO `BIST_CTRL` bit 2.
- Removed: SYSTIMER `CONF` bit 0, LP_SYSTEM `SYS_CTRL` bits 22, 23, 25 and 26, HP_SYSTEM `RNG_CFG`
  bits 24-31, SPI_MEM_C/S `CTRL1` bit 23, GPIO input-select registers for signals 138 and 139,
  eFuse `CONF` bits 16-19 and `STATUS` bits 20-23.
- Resized: SPI_MEM_C/S `FSM` lock delay (5 to 12 bits), `XTS_PHYSICAL_ADDRESS` (26 to 30 bits).
- eFuse: the repeat-data words, the repeat-error words `RD_REPEAT_ERR0`-`4`, `RD_MAC_SYS_5` and
  system part 2 data 3/6/7 are laid out anew; the MAC words and `RD_MAC_SYS_2` (wafer and block
  revisions), which the model writes, are not.

None of it is in a field the model gives a meaning to; the interrupt matrix keeps six bits of each
map register, so the TEE flags read back 0. This is why hello_world and ESP-Hosted run on the rev
>= 3 model with the rev < 3 peripherals.

## Between headers

The comparison above pairs each header with its namesake, so a register that IDF reaches through a
different header on rev >= 3 does not show in it. `hw_ver3` has 23 headers `hw_ver1` lacks
(`diff <(ls hw_ver1/soc) <(ls hw_ver3/soc)`): new blocks (TRNG, EMAC, the PMS and AXI performance
monitor blocks, `mem_monitor` for the L2MEM and TCM monitors) and `*_eco5_*` variants. Of the
variants, only `ecdsa_ll.h` includes one (`grep -rl eco5` over `components/hal`, `esp_hw_support`
and `bootloader_support`), and the model has no ECDSA. One alias does move: `wdev_reg.h` defines `WDEV_RND_REG`, the random number that
`esp_random` and the bootloader read, as LP_SYSTEM `RNG_DATA` (0x501101A4, `hw_ver1/soc/wdev_reg.h:16`)
on rev < 3 and as LP_TRNG `RNG_DATA` (0x50126004, `hw_ver3/soc/wdev_reg.h:16`, `trng_reg.h:52`) on
rev >= 3. Until #291's review the model had only the first, so the rev >= 3 builds read register RAM
there (zero); it now has both.

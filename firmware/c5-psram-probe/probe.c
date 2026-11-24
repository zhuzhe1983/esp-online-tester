/* Original C5 SRAM diagnostic. No IDF or manufacturer binary is patched.
 * IDF v5.5.4 C5 MSPI/IO_MUX headers and AP APS6404L-3SQR rev2.7 protocol.
 * All device modes, physical pads and MMU entries are guest-configured. */
typedef unsigned int u32;
typedef unsigned short u16;
typedef unsigned char u8;
#define R(a) (*(volatile u32 *)(a))
#define SPI 0x60003000u
#define RAM 0x42000000u
static void puts(const char *s) { while (*s) R(0x60000000u) = (u8)*s++; }
static void fail(void) { puts("C5 PSRAM8 FAIL\n"); for (;;) __asm__ volatile("wfi"); }
static void pause_cycles(u32 n) {
    u32 start, now; __asm__ volatile("rdcycle %0" : "=r"(start));
    do { __asm__ volatile("rdcycle %0" : "=r"(now)); } while (now - start < n);
}
static u32 command(u32 opcode, int qpi, int address, u32 addr, int write, u32 value, u32 read_bytes, u32 dummy) {
    R(SPI + 8) = qpi ? (1u << 8) | (1u << 13) | (1u << 24) : 0;
    R(SPI + 4) = addr;
    R(SPI + 0x18) = (1u << 31) | (address ? 1u << 30 : 0) | (write ? 1u << 27 : 0)
        | (read_bytes ? 1u << 28 : 0) | (dummy ? 1u << 29 : 0) | (qpi ? 1u << 15 : 0);
    R(SPI + 0x1c) = (23u << 26) | (dummy ? dummy - 1 : 0);
    R(SPI + 0x20) = (7u << 28) | opcode;
    R(SPI + 0x24) = 31; R(SPI + 0x28) = read_bytes ? read_bytes * 8 - 1 : 0;
    R(SPI + 0x58) = value; R(SPI) = 1u << 18;
    while (R(SPI) & (1u << 18)) {}
    return R(SPI + 0x58);
}
int main(void) {
    static const u32 pads[6] = {15, 17, 18, 20, 21, 22};
    for (u32 n = 0; n < 6; ++n) R(0x60090000u + pads[n] * 4) = 0x800;
    R(0x60096018u) = 5; R(0x6009601cu) = 0x400; /* nominal 48 MHz XTAL */
    R(SPI + 0x34) = 1; R(SPI + 0x14) = 0x30103; /* only CS1; 12 MHz */
    pause_cycles(36000); /* nominal 150 us at the emulator's 240 MHz CPU */
    command(0xf5, 1, 0, 0, 0, 0, 0, 0);
    command(0x66, 0, 0, 0, 0, 0, 0, 0); command(0x99, 0, 0, 0, 0, 0, 0, 0); pause_cycles(24);
    if ((command(0x9f, 0, 1, 0, 0, 0, 3, 0) & 0xffffff) != 0x425d0d) fail();
    command(0x02, 0, 1, 0x100, 1, 0x5a6b7c8d, 0, 0);
    if (command(0x03, 0, 1, 0x100, 0, 0, 4, 0) != 0x5a6b7c8d) fail();
    command(0x35, 0, 0, 0, 0, 0, 0, 0);
    command(0x38, 1, 1, 0x104, 1, 0x89abcdef, 0, 0);
    if (command(0xeb, 1, 1, 0x104, 0, 0, 4, 6) != 0x89abcdef) fail();
    R(0x60002040u) = (1u << 20) | (23u << 14) | (5u << 6) | 0x34;
    R(0x60002048u) = (7u << 28) | 0xeb; R(0x6000204cu) = (7u << 28) | 0x38;
    R(0x60002050u) = 1u << 31; /* cache PSRAM clock = 48 MHz XTAL */
    R(0x6000203cu) = 0; /* open the AXI interface; hardware POR closes it */
    R(0x60002384u) = 4; /* MMU powered, 64 KiB pages */
    for (u32 page = 0; page < 128; ++page) { R(0x60002380u) = page; R(0x6000237cu) = 0x600 | page; }
    volatile u32 *words = (volatile u32 *)RAM;
    for (u32 n = 0; n < 2097152; ++n) words[n] = n ^ 0x5a6b7c8d;
    for (u32 n = 0; n < 2097152; ++n) if (words[n] != (n ^ 0x5a6b7c8d)) fail();
    *(volatile u8 *)(RAM + 0x7fffff) = 0xa5;
    *(volatile u16 *)(RAM + 0x7ffffc) = 0x1234;
    if (*(volatile u8 *)(RAM + 0x7fffff) != 0xa5 || *(volatile u16 *)(RAM + 0x7ffffc) != 0x1234) fail();
    words[0] = 0x02500513; words[1] = 0x00008067; /* addi a0,zero,37; ret */
    __asm__ volatile("fence.i" ::: "memory");
    if (((int (*)(void))RAM)() != 37) fail();
    puts("C5 PSRAM8 PASS\n"); return 0;
}

/* S31 IDF v6.1 spi1_mem_c_reg.h: explicit test NOR, SPI1 CS0 commands.
 * JEDEC [ef,40,18] is fixture input only, not a claim about INK4 Plus hardware.
 * Array bytes are supplied independently and altered in a negative control.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20501000
    li t0, 0x10000000         /* RDID */
    sw t0, 0(s1)
    lw t0, 0(s1)
    bnez t0, fail
    lw t0, 0x58(s1)
    li t1, 0x1840ef
    bne t0, t1, fail
    li t0, 0x40000000         /* WREN */
    sw t0, 0(s1)
    li t0, 0x08000000         /* RDSR into RD_STATUS */
    sw t0, 0(s1)
    lw t0, 0x2c(s1)
    li t1, 2
    bne t0, t1, fail
    li t0, 0x20000000         /* WRDI */
    sw t0, 0(s1)
    li t0, 0x08000000
    sw t0, 0(s1)
    lw t0, 0x2c(s1)
    bnez t0, fail
    li t0, 0x20
    sw t0, 4(s1)
    li t0, 31
    sw t0, 0x28(s1)
    li t0, 0xd0000000         /* command+address+MISO */
    sw t0, 0x18(s1)
    li t0, 0x5c000000         /* 24 address bits */
    sw t0, 0x1c(s1)
    li t0, 0x70000003         /* 8-bit 03h ordinary read */
    sw t0, 0x20(s1)
    li t0, 0x40000
    sw t0, 0(s1)
    lw t0, 0x58(s1)
    li t1, 0x17293c5a
    bne t0, t1, fail
    li t0, 0x24
    sw t0, 4(s1)
    li t0, 0x80000000         /* dedicated read, same backing array */
    sw t0, 0(s1)
    lw t0, 0x58(s1)
    li t1, 0x87654321
    bne t0, t1, fail
    la s0, pass
    jal ra, puts
1:  wfi
    j 1b
fail:
    la s0, failed
    jal ra, puts
    ebreak
puts:
    li t3, 0x2038a000
2:  lbu t4, 0(s0)
    beqz t4, 3f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 2b
3:  ret
.section .rodata,"a",@progbits
pass: .asciz "S31 MSPI ID STATUS ARRAY PASS\n"
failed: .asciz "S31 MSPI FAIL\n"

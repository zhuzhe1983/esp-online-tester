/* Fixed S31 IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643 LPCON/UART clocks.
 * Guest executes real I2C reads/writes/reset with all LP clock bits populated.
 * Host supplies BIAS bytes and enable-word input; results/PASS are guest output.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x2010f800
    li s2, 0x2010f000
    li t3, 0x2f070000
    lw t0, 0(t3)               /* explicit input: software EN bits */
    sw t0, 0x18(s2)
    li t0, 31
    sw t0, 0x1c(s2)            /* FO cannot substitute for missing I2C EN */
    li t0, 0x274               /* BLE: XTAL /40 */
    sw t0, 4(s2)
    li t0, 0x31                /* COEX: nominal RC_SLOW /4 */
    sw t0, 8(s2)
    li t0, 0x134               /* Wi-Fi power: XTAL /20 */
    sw t0, 12(s2)
    li s3, 0x20587000
    li t0, 0x02010181          /* UART1 APB off, core XTAL /2.5 */
    sw t0, 0x8c(s3)
    lw t1, 0x8c(s3)
    bne t0, t1, fail
    li t0, 0x87                /* UART2 APB on, core reset */
    sw t0, 0x90(s3)
    li t0, 0x03                /* UART3 APB on, core gate off */
    sw t0, 0x94(s3)
    li t3, 0x20589010
    sw zero, 0(t3)
    lw t1, 0(t3)
    bnez t1, fail
    lw t1, 4(t3)
    li t0, 0x80000000
    bne t0, t1, fail
    li t1, 0x20109c04
    lw t0, 0(t1)
    li t2, 0x1000
    or t0, t0, t2
    sw t0, 0(t1)
    li t0, 0xffffbf
    sw t0, 0x1c(s1)
    li t0, 0x6a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0x17
    bne t1, t2, fail
    li t0, 0x01a7006a
    sw t0, 4(s1)
    li t0, 0x6a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0xa7
    bne t1, t2, fail
    li t0, 0x16a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0x29
    bne t1, t2, fail
    li t0, 0x012a016a
    sw t0, 4(s1)
    li t0, 4
    sw t0, 0x24(s2)             /* reset digital master, preserve analog data */
    sw zero, 0x24(s2)
    li t0, 0x6a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0xa7
    bne t1, t2, fail
    li t0, 0x16a
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0x2a
    bne t1, t2, fail
    li t0, 0x104                /* route BIAS to host0; same slave storage */
    sw t0, 0x20(s1)
    li t0, 0x6a
    sw t0, 0(s1)
    lw t1, 0(s1)
    srli t1, t1, 16
    li t2, 0xa7
    bne t1, t2, fail
    li t0, 0x2f060000
    li t1, 1
    sw t1, 0(t0)
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
pass: .asciz "S31 MODEM CLOCKS I2C RESET PASS\n"
failed: .asciz "S31 MODEM CLOCKS FAIL\n"

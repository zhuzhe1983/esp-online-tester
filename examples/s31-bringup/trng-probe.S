/* S31 functional CRC random-data gates: ESP-IDF v6.1 trng_ll.h/trng_reg.h and
 * lp_peri_clkrst_reg.h. Tests the real MMIO path; not an entropy/cryptographic test.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20814000
    li s2, 0x2071003c
    lw t0, 0x44(s1)
    bnez t0, fail
    li t0, 0x10000000
    sw t0, 0xfc(s1)
    li t0, 0xc0000000
    sw t0, 0(s2)
    li t0, 0x40000000
    sw t0, 0(s2)
    lw t1, 0x44(s1)
    bnez t1, fail
    li t0, 0xc0000000
    sw t0, 0(s1)
    lw s3, 0x44(s1)
    beqz s3, fail
    lw s4, 0x48(s1)
    beqz s4, fail
    beq s3, s4, fail
    sw zero, 0(s2)
    lw t1, 0x48(s1)
    bne t1, s4, fail
    li t0, 0x40000000
    sw t0, 0(s2)
    sw t0, 0(s1)
    lw t1, 0x44(s1)
    bne t1, s4, fail
    li t0, 0xc0000000
    sw t0, 0(s1)
    sw zero, 0xfc(s1)
    lw t1, 0x44(s1)
    bne t1, s4, fail
    li t0, 0xc0000000
    sw t0, 0(s2)
    lw t1, 0x44(s1)
    bnez t1, fail
    la s0, pass
    jal ra, puts
halt:
    wfi
    j halt
fail:
    la s0, failed
    jal ra, puts
    ebreak
puts:
    li t3, 0x2038a000
1:  lbu t4, 0(s0)
    beqz t4, 2f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 1b
2:  ret
.section .rodata,"a",@progbits
pass: .asciz "S31 TRNG GATES PASS\n"
failed: .asciz "S31 TRNG FAIL\n"

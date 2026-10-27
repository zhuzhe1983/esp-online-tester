/* ESP-IDF6.1 S31 SFDP24+dummy8 and MSPI BBPLL480/6 sequence.
 * Caller supplies an explicit SFDP interval0x10=[12,34,56,78] and stable480MHz
 * source. These are fixture inputs, not the INK4 Plus BOM or oscillator lock. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20501000
    li s2, 0x20587064
    li t0, 0xf0000000
    sw t0, 0x18(s1)
    li t0, 0x7000005a
    sw t0, 0x20(s1)
    li t0, 0x10
    sw t0, 4(s1)
    li t0, 31
    sw t0, 0x28(s1)
    li s3, 0x40000
    sw s3, 0(s1)
    lw t0, 0x58(s1)
    li t1, 0x78563412
    bne t0, t1, fail
    li t0, 0x12
    sw t0, 4(s1)
    li t0, 15
    sw t0, 0x28(s1)
    sw s3, 0(s1)
    lw t0, 0x58(s1)
    li t1, 0x7856
    bne t0, t1, fail
    li t0, 0xba1
    sw t0, 0(s2)
    li t0, 0x80000000
    sw t0, 0x18(s1)
    li t0, 0x7000007a
    sw t0, 0x20(s1)
    li t0, 0x2f070000
    lw t1, 0(t0)
    beqz t1, 1f
    li t0, 0x20589000
    lw t1, 0(t0)
    li t2, 0xbfffffff
    and t1, t1, t2
    sw t1, 0(t0)
1:  sw s3, 0(s1)        /* idle Resume; with source gated this must fault */
    la s0, pass
    jal ra, puts
2:  wfi
    j 2b
fail:
    la s0, failed
    jal ra, puts
    ebreak
puts:
    li t3, 0x2038a000
3:  lbu t4, 0(s0)
    beqz t4, 4f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 3b
4:  ret
.section .rodata,"a",@progbits
pass: .asciz "S31 SFDP FLASH80 IDLE PASS\n"
failed: .asciz "S31 FLASH PARAMS FAIL\n"

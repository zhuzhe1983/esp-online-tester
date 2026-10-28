/* S31 IDF v6.1 PERIF internal-I2C power/reset and BOD mode1 fixture.
 * Host supplies reg10=0xf7 and a BOD enable mailbox, then qualified comparator input.
 * No analog threshold, glitch waveform or silicon reset measurement is implied. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x2010f800
    li t0, 0x2010f018
    li t1, 4
    sw t1, 0(t0)
    li s2, 0x20704208
    lw t0, 0(s2)
    li t1, 0x40000000
    bne t0, t1, fail
    li t0, 0xc0000000
    sw t0, 0(s2)
    li t0, 0xa69
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0xf7
    bne t1, t2, fail
    andi t1, t1, -14
    slli t1, t1, 16
    li t0, 0x01000a69
    or t0, t0, t1
    sw t0, 4(s1)
    li t0, 0xa69
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0xf2
    bne t1, t2, fail
    li t0, 0x40000000
    sw t0, 0(s2)
    li t0, 0xc0000000
    sw t0, 0(s2)
    li t0, 0xa69
    sw t0, 4(s1)
    lw t1, 4(s1)
    srli t1, t1, 16
    li t2, 0xf7
    bne t1, t2, fail
    li s3, 0x20702000
    lw t0, 0x1c(s3)
    andi t0, t0, -3
    sw t0, 0x1c(s3)
    li t0, 0x2f070000
    lw t1, 0(t0)
    sw t1, 4(s3)
    lw t2, 4(s3)
    bne t1, t2, fail
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
pass: .asciz "S31 PERIF RESET BOD ARMED\n"
failed: .asciz "S31 POWER FAIL\n"

/* Fixed IDF v6.1 S31 CPLL digital sequence and actual ROM frequency reader.
 * Host supplies DIG bytes [0x11,0x22,0xff,0x44] and a control mailbox:
 * 0 normal,1 missing power,2 invalid divider,3 no start,4 power loss while active.
 * Calibration polling checks a functional completion, not physical settling time. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07cfb0
    li t0, 0x2f070000
    lw s4, 0(t0)
    li s1, 0x20587000
    li s2, 0x20701000
    li s3, 0x20704000
    li t0, 0x2010f018
    li t1, 4
    sw t1, 0(t0)
    li t0, 0x2010f804
    li t1, 0xd6d
    sw t1, 0(t0)
    lw t1, 0(t0)
    srli t1, t1, 16
    li t2, 255
    bne t1, t2, fail
    andi t1, t1, -13
    slli t1, t1, 16
    li t2, 0x01000d6d
    or t1, t1, t2
    sw t1, 0(t0)
    li t1, 0xd6d
    sw t1, 0(t0)
    lw t1, 0(t0)
    srli t1, t1, 16
    li t2, 0xf3
    bne t1, t2, fail
    li t0, 8
    sw t0, 0x174(s1)
    li t0, 0x81
    li t1, 2
    bne s4, t1, 1f
    li t0, 0
1:  sw t0, 0x4c(s2)
    li t1, 1
    beq s4, t1, 2f
    li t0, 0x08880000
    sw t0, 0xf4(s3)
2:  li t1, 3
    beq s4, t1, 3f
    sw zero, 0x174(s1)
3:  li t2, 100
4:  lw t0, 0x174(s1)
    andi t0, t0, 4
    bnez t0, 5f
    addi t2, t2, -1
    bnez t2, 4b
    la s0, timeout
    jal ra, puts
    j done
5:  li t0, 8
    sw t0, 0x174(s1)
    li t0, 1
    sw t0, 0(s1)
    sw t0, 0x10(s1)
    li t0, 3
    sw t0, 4(s1)
    sw zero, 8(s1)
    sw zero, 0xc(s1)
    li t0, 1
    sw t0, 0x14(s1)
    li t0, 0x2f806ba2
    jalr ra, t0
    li t1, 80000000
    bne a0, t1, fail
    li t1, 4
    bne s4, t1, 6f
    li t0, 0x111
    sw t0, 0xf4(s3)      /* must fault; cannot keep executing without active CPU clock */
    j fail
6:  sw zero, 0(s2)       /* RTC_FAST=RC_FAST */
    sw zero, 0xd0(s3)    /* remove its actual oscillator gate */
    lw t0, 0xd0(s3)
    bnez t0, fail
    la s0, pass
    jal ra, puts
    j done
fail:
    la s0, failed
    jal ra, puts
    ebreak
done:
    wfi
    j done
puts:
    li t3, 0x2038a000
7:  lbu t4, 0(s0)
    beqz t4, 8f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 7b
8:  ret
.section .rodata,"a",@progbits
pass: .asciz "S31 ROM CPLL80 DIG PASS\n"
timeout: .asciz "S31 CPLL NOT READY\n"
failed: .asciz "S31 CPLL FAIL\n"

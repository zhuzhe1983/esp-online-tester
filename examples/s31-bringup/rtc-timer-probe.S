/* IDF6.1 S31 rtc_timer_ll.h snapshot then paired low/high reads.
 * Mode0: explicit slow-clock input. Mode1: optional interpreter-clock profile.
 * Guest checks latched coherence, update-done clear and real reset state. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20800000
    li t0, 0x2f070000
    lw s3, 0(t0)
    li t0, 1000
1:  addi t0, t0, -1
    bnez t0, 1b
    lw t0, 0x28(s1)
    bnez t0, fail
    li t0, 0x08000000
    sw t0, 0x10(s1)
    lw t0, 0x10(s1)
    bnez t0, fail
    lw s4, 0x14(s1)
    lw s5, 0x18(s1)
    bnez s3, timed
    li t0, 0xfffffffe
    bne s4, t0, fail
    li t0, 1
    bne s5, t0, fail
    j check
timed:
    beqz s4, fail
    bnez s5, fail
check:
    lw t0, 0x1c(s1)
    bne t0, s4, fail
    lw t0, 0x20(s1)
    bne t0, s5, fail
    li t1, 0x20000000
    lw t0, 0x28(s1)
    bne t0, t1, fail
    sw t1, 0x34(s1)
    lw t0, 0x28(s1)
    bnez t0, fail
    lw t0, 0x38(s1)
    bne t0, t1, fail
    li t2, 0x2070100c
    li t0, 0x80000000
    sw t0, 0(t2)
    li t0, 0x08000000
    sw t0, 0x10(s1)
    lw t0, 0x14(s1)
    bnez t0, fail
    lw t0, 0x28(s1)
    bnez t0, fail
    la s0, pass_text
    jal ra, puts
wait:
    wfi
    j wait
fail:
    la s0, fail_text
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
pass_text: .asciz "S31 RTC SNAPSHOT PASS\n"
fail_text: .asciz "S31 RTC SNAPSHOT FAIL\n"

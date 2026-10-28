/* S31 LP_AONCLKRST per-core gate/WT reset paths, IDF6.1 lp_clkrst_reg.h.
 * Mailbox0..3 selects RTC-WDT receiver gates;4/5 requests software reset;
 * 6/7 enables core0/1 lockup gate, with an explicit qualified host event. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li t0, 0x2f070000
    lw s3, 0(t0)
    li s1, 0x2070103c
    li t0, 4
    bgeu s3, t0, special
    andi t0, s3, 1
    slli t0, t0, 19
    li t1, 0x40010000
    or t0, t0, t1
    sw t0, 0(s1)
    andi t0, s3, 2
    slli t0, t0, 18
    or t0, t0, t1
    sw t0, 4(s1)
    li t2, 0x20801000
    li t0, 0x50d83aa1
    sw t0, 0x18(t2)
    li t0, 1
    sw t0, 4(t2)
    li t0, 3
    sw t0, 8(t2)
    li t0, 0xa6000c00
    sw t0, 0(t2)
    sw zero, 0x18(t2)
    j ready
special:
    andi t0, s3, 1
    slli t0, t0, 2
    add s1, s1, t0
    li t0, 6
    bltu s3, t0, ready
    li t0, 0xc0010000
    sw t0, 0(s1)
ready:
    la s0, ready_text
    jal ra, puts
    li t0, 4
    bltu s3, t0, wait
    li t0, 6
    bgeu s3, t0, wait
    li t0, 0x40110000
    sw t0, 0(s1)
    la s0, fail_text
    jal ra, puts
    ebreak
wait:
    wfi
    j wait
puts:
    li t3, 0x2038a000
1:  lbu t4, 0(s0)
    beqz t4, 2f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 1b
2:  ret
.section .rodata,"a",@progbits
ready_text: .asciz "S31 RESET READY\n"
fail_text: .asciz "S31 RESET MISSED\n"

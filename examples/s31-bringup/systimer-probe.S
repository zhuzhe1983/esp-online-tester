/* Fixed S31 IDF6.1 SYSTIMER_CTRL0/base, source33, CLIC ID17. Genuine
 * counter snapshot -> programmed alarm -> WFI -> CLIC ISR -> clear -> mret.
 * No clock input leaves the first snapshot pending; WFI needs explicit ticks. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f06f000
    li s1, 0x20399000
    li t0, 0x20587120
    li t1, 0x11
    sw t1, 0(t0)
    li t1, 0x40000000
    sw t1, 4(s1)
1:  lw t0, 4(s1)
    li t1, 0x20000000
    and t0, t0, t1
    beqz t0, 1b
    lw t0, 0x44(s1)
    beqz t0, unexpected
    addi t0, t0, 512
    sw zero, 0x1c(s1)
    sw t0, 0x20(s1)
    li t0, 1
    sw t0, 0x50(s1)
    sw t0, 0x64(s1)
    li t0, 0x41000000
    sw t0, 0(s1)
    la t0, unexpected
    ori t0, t0, 3
    csrw mtvec, t0
    la t0, vector_table
    csrw 0x307, t0
    li t0, 0x20585084
    li t1, 0x211
    sw t1, 0(t0)
    li t0, 0x10801044
    li t1, 0x3f010100
    sw t1, 0(t0)
    li t0, 0x10800000
    li t1, 7
    sw t1, 0(t0)
    li t0, 0x1f
    csrw 0x347, t0
    csrsi mstatus, 8
wait:
    li t0, 0x2f060000
    lw t1, 0(t0)
    bnez t1, done
    wfi
    j wait
done:
    csrci mstatus, 8
    csrr t0, 0xfb1
    bnez t0, unexpected
    lw t0, 0x68(s1)
    bnez t0, unexpected
    la s0, pass
    call puts
halt:
    wfi
    j halt
timer_irq:
    lw t0, 0x70(s1)
    li t1, 1
    bne t0, t1, unexpected
    sw t1, 0x68(s1)             /* RAW WTC belongs to CLR, not writable status */
    lw t0, 0x70(s1)
    bne t0, t1, unexpected
    sw t1, 0x6c(s1)
    lw t0, 0x70(s1)
    bnez t0, unexpected
    li t0, 0x2f060000
    sw t1, 0(t0)
    mret
.balign 64
unexpected:
    la s0, failed
    call puts
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
.balign 64
vector_table:
.rept 17
    .word unexpected
.endr
    .word timer_irq
.rept 30
    .word unexpected
.endr
pass: .asciz "S31 SYSTIMER CLOCK SNAPSHOT IRQ MRET PASS\n"
failed: .asciz "S31 SYSTIMER FAIL\n"

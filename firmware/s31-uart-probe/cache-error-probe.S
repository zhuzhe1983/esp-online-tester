/* Real source69 cache error -> hart0 CLIC ISR, caused by hart1's executed
 * load through a closed shared D-cache bus. S31 IDF6.1 fixed cache LL/regs.
 * The diagnostic runner stops at the synchronous guest access exception; the
 * fixture test resumes normally so both actual exception/IRQ handlers execute. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f06f000
    li s1, 0x2c000000
    li s0, 0x2f060000
    sw zero, 0(s0)
    li t0, 0x20500000
    li t1, 0x40000001
    sw t1, 0x3c(t0)
    sw zero, 0x380(t0)
    li t1, 0x1000
    sw t1, 0x37c(t0)
    li t0, 0x13
    sw t0, 0x16c(s1)             /* real failure propagation enabled */
    li t0, 1
    sw t0, 0x3d8(s1)
    li t0, 31
    sw t0, 0x174(s1)
    sw t0, 0x170(s1)
    li t0, 0x20585114             /* source69 */
    li t1, 0x211
    sw t1, 0(t0)
    li t0, 0x10801044
    li t1, 0x3f010100
    sw t1, 0(t0)
    li t0, 0x10800000
    li t1, 7
    sw t1, 0(t0)
    la t0, unexpected
    ori t0, t0, 3
    csrw mtvec, t0
    la t0, vector_table
    csrw 0x307, t0
    li t0, 0x1f
    csrw 0x347, t0
    csrsi mstatus, 8
    la a0, secondary
    li t0, 0x2f8000a8
    jalr ra, t0, 0
    li t0, 0x207041ec
    li t1, 0x00ff0000
    sw t1, 0(t0)
    li t0, 0x20587020
    li t1, 3
    sw t1, 0(t0)
wait:
    lw t0, 0(s0)
    bnez t0, done
    wfi
    j wait
done:
    lw t0, 0x178(s1)
    bnez t0, unexpected
    la s0, pass_msg
    call puts
    csrci mstatus, 8
1:  wfi
    j 1b
secondary:
    li sp, 0x2f06e000
    la t0, secondary_exception
    csrw mtvec, t0
    li t0, 0x2c000004
    li t1, 1
    sw t1, 0(t0)                 /* close shared DBUS0 */
    li t0, 0x40000000
.global fault_load
fault_load:
    lw t1, 0(t0)                 /* this instruction must fault */
    j unexpected
.balign 64
secondary_exception:
    csrr t0, mcause
    li t1, 5
    bne t0, t1, unexpected
    csrr t0, mtval
    li t1, 0x40000000
    bne t0, t1, unexpected
    wfi
    j .
cache_irq:
    li t0, 0x2c000000
    lw t1, 0x17c(t0)
    li t2, 16
    bne t1, t2, unexpected
    /* Direct RAW/status writes cannot forge or clear the latched event. */
    sw t2, 0x178(t0)
    lw t1, 0x178(t0)
    bne t1, t2, unexpected
    sw t2, 0x174(t0)
    lw t1, 0x17c(t0)
    bnez t1, unexpected
    sw zero, 4(t0)
    li t0, 0x2f060000
    li t1, 1
    sw t1, 0(t0)
    mret
.balign 64
unexpected:
    la s0, fail_msg
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
    .word cache_irq
.rept 30
    .word unexpected
.endr
pass_msg: .asciz "S31 CACHE ERROR REAL ISR PASS\n"
fail_msg: .asciz "S31 CACHE ERROR FAIL\n"

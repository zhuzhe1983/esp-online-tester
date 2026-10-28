/* S31 IDF v6.1 bus_monitor/interrupt_core0/hp_sys_clkrst headers.
 * Real guest PC/SP capture, stack-bound IRQ119 -> CLIC17, and two actual faults.
 * Host resumes at the guest's architectural trap vector after each diagnostic stop;
 * no PC/exception data is injected by the fixture harness.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07f000
    li s1, 0x2d002000
    li t0, 0x20587048
    li t1, 1
    sw t1, 0(t0)
    li t1, 3
    sw t1, 0x44(s1)
    la t0, capture
capture:
    lw t1, 0x48(s1)
    bne t0, t1, fail
    lw t1, 0x4c(s1)
    bne sp, t1, fail
    addi t0, sp, -16
    sw t0, 0x38(s1)
    addi t0, sp, 16
    sw t0, 0x3c(s1)
    li t0, 0x1300
    sw t0, 0(s1)
    li t0, 0x300
    sw t0, 8(s1)
    la t0, exception_handler
    ori t0, t0, 3
    csrw mtvec, t0
    la t0, vector_table
    csrw 0x307, t0
    li t0, 0x205851dc            /* source119 */
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
    li s2, 0
    li s3, 0
    csrsi mstatus, 8
    addi sp, sp, -32
    nop
    li t0, 1
    bne s2, t0, fail
    csrci mstatus, 8
    li t0, 0x1000
    sw t0, 0(s1)              /* exception trace only */
    lw t0, 4(s1)
    bnez t0, fail
    .balign 4
fault_load:
    lw t2, 0(zero)
    .balign 4
fault_store:
    sw zero, 0(zero)
    li t0, 2
    bne s3, t0, fail
    lw t0, 0x68(s1)
    li t1, 7
    bne t0, t1, fail
    lw t0, 0x70(s1)
    la t1, fault_store
    bne t0, t1, fail
    lw t0, 0x78(s1)
    li t1, 5
    bne t0, t1, fail
    lw t0, 0x80(s1)
    la t1, fault_load
    bne t0, t1, fail
    lw t0, 0x6c(s1)
    bnez t0, fail
    lw t0, 0xd0(s1)          /* inactive hart1 must not acquire hart0 samples */
    bnez t0, fail
    la s0, pass
    jal ra, puts
halt:
    wfi
    j halt
stack_irq:
    addi sp, sp, 32
    lw t0, 4(s1)
    li t1, 0x100
    bne t0, t1, fail
    sw t1, 0xc(s1)
    addi s2, s2, 1
    mret
.balign 64
exception_handler:
    csrr t0, mepc
    addi t0, t0, 4
    csrw mepc, t0
    addi s3, s3, 1
    mret
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
.balign 64
vector_table:
.rept 17
    .word fail
.endr
    .word stack_irq
.rept 30
    .word fail
.endr
pass: .asciz "S31 MONITOR CAPTURE IRQ TRAPS PASS\n"
failed: .asciz "S31 MONITOR FAIL\n"

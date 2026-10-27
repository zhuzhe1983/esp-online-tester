/* SRAM source fixture for S31 UART0 -> matrix -> standard CLIC -> guest ISR -> mret.
 * ESP-IDF v6.1 S31 uart_reg.h, interrupt_core0_reg.h, clic_reg.h, interrupt_reg.h,
 * and riscv/csr_clic.h: source9, M pass level2, CLIC ID17, mintstatus0xfb1/mintthresh0x347.
 * Like the polled fixture, this assumes the UART clock/pins are already usable.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    la t0, unexpected
    ori t0, t0, 3
    csrw mtvec, t0
    la t0, vector_table
    csrw 0x307, t0
    li t0, 0xff
    csrw 0x347, t0
    li t0, 0x2038a000
    li t1, 1
    sw t1, 0x24(t0)
    sw t1, 0xc(t0)
    li t0, 0x20585024
    li t1, 0x211
    sw t1, 0(t0)
    li t0, 0x10801044
    li t1, 0x3f010100
    sw t1, 0(t0)
    li t0, 0x10800000
    li t1, 7
    sw t1, 0(t0)
    la t0, handled
    sw zero, 0(t0)
    la s0, ready
    jal ra, puts
    li t0, 0x1f
    csrw 0x347, t0
    csrsi mstatus, 8
wait:
    la t0, handled
    lw t1, 0(t0)
    bnez t1, done
    wfi
    j wait
done:
    csrr t0, 0xfb1
    bnez t0, unexpected
    la s0, pass
    jal ra, puts
    csrci mstatus, 8
halt:
    wfi
    j halt
uart_irq:
    csrr t0, 0xfb1
    li t1, 0x3f000000
    bne t0, t1, unexpected
    li t0, 0x2038a000
    lw t1, 0(t0)
    li t2, 65
    bne t1, t2, unexpected
    sw t1, 0(t0)
    li t1, 10
    sw t1, 0(t0)
    li t1, 1
    sw t1, 0x10(t0)
    la t0, handled
    sw t1, 0(t0)
    mret
.balign 64
unexpected:
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
    .word unexpected
.endr
    .word uart_irq
.rept 30
    .word unexpected
.endr
ready: .asciz "S31 IRQ READY\n"
pass: .asciz "S31 IRQ MRET PASS\n"
failed: .asciz "S31 IRQ FAIL\n"
.section .bss,"aw",@nobits
.balign 4
handled: .space 4

/* S31 USB Serial/JTAG EP1: actual full packet, host-driven backpressure, explicit ZLP,
 * RX source2 -> matrix -> standard CLIC -> guest ISR -> mret. IDF v6.1 S31
 * usb_serial_jtag_reg.h/LL: 64-byte auto-flush; WR_DONE after draining emits ZLP.
 * Functional packet peer, no enumeration, PHY or USB frame timing assumption.
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
    li t0, 0x1f
    csrw 0x347, t0
    li s1, 0x20391000
    li t0, 0
    li t1, 64
1:  sw t0, 0(s1)
    addi t0, t0, 1
    blt t0, t1, 1b
    lw t0, 4(s1)
    andi t0, t0, 2
    bnez t0, unexpected
    la s0, full
    jal ra, puts
wait_full:
    lw t0, 4(s1)
    andi t0, t0, 2
    beqz t0, wait_full
    li t0, 1
    sw t0, 4(s1)
wait_zlp:
    lw t0, 4(s1)
    andi t0, t0, 2
    beqz t0, wait_zlp
    li t0, 0x20585008
    li t1, 0x211
    sw t1, 0(t0)
    li t0, 0x10801044
    li t1, 0x3f010100
    sw t1, 0(t0)
    li t0, 0x10800000
    li t1, 7
    sw t1, 0(t0)
    li t0, 4
    sw t0, 0x10(s1)
    la t0, handled
    sw zero, 0(t0)
    la s0, ready
    jal ra, puts
    csrsi mstatus, 8
wait_rx:
    la t0, handled
    lw t1, 0(t0)
    bnez t1, done
    wfi
    j wait_rx
done:
    csrr t0, 0xfb1
    bnez t0, unexpected
    la s0, pass
    jal ra, puts
    csrci mstatus, 8
halt:
    wfi
    j halt
usb_irq:
    csrr t0, 0xfb1
    li t1, 0x3f000000
    bne t0, t1, unexpected
    li t0, 0x20391000
    lw t1, 4(t0)
    andi t1, t1, 4
    beqz t1, unexpected
    lw t1, 0(t0)
    li t2, 65
    bne t1, t2, unexpected
    sw t1, 0(t0)
    li t1, 1
    sw t1, 4(t0)
    li t1, 4
    sw t1, 0x14(t0)
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
    .word usb_irq
.rept 30
    .word unexpected
.endr
full: .asciz "S31 USB FULL WAIT\n"
ready: .asciz "S31 USB RX READY\n"
pass: .asciz "S31 USB IRQ ECHO PASS\n"
failed: .asciz "S31 USB FAIL\n"
.section .bss,"aw",@nobits
.balign 4
handled: .space 4

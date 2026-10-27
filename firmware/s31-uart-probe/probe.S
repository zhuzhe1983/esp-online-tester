/* SRAM-linked S31 diagnostic, not an ESP-IDF boot image.
 * ESP-IDF v6.1: soc/esp32s31/include/soc/soc.h and register/soc/{reg_base,uart_reg}.h.
 * UART0=0x2038a000, FIFO=0, STATUS=0x1c, RXFIFO_CNT bits7:0.
 * This subset assumes a debugger-loaded SRAM program and an already usable UART clock.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    la s0, ready
    jal ra, puts
    li t3, 0x2038a000
1:  lw t0, 0x1c(t3)
    andi t0, t0, 255
    beqz t0, 1b
    lw t0, 0(t3)
    sw t0, 0(t3)
    li t1, 65
    bne t0, t1, fail
    li t0, 10
    sw t0, 0(t3)
    li t0, 0x2f070000
    li t1, 0x5a1234a5
    sw t1, 0(t0)
    lw t2, 0(t0)
    bne t1, t2, fail
    li t0, 0x2000
    csrs mstatus, t0
    li t0, 0x3fc00000
    fmv.w.x f0, t0
    fadd.s f1, f0, f0
    fmv.x.w t1, f1
    li t2, 0x40400000
    bne t1, t2, fail
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
ready: .asciz "S31 UART READY\n"
pass: .asciz "S31 SRAM FPU PASS\n"
failed: .asciz "S31 FAIL\n"

/* S31 RTC WDT source 136 -> CLIC ID18 -> guest handler, then stage1 system reset.
 * IDF v6.1 rtc_wdt_reg.h, rwdt_ll.h, interrupt_core0_reg.h. Host supplies explicit
 * slow-clock ticks; delay_sel=0 gives the documented x2 stage0 multiplier.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    la t0, failed
    ori t0, t0, 3
    csrw mtvec, t0
    la t0, vector_table
    csrw 0x307, t0
    li t0, 0x10800000
    li t1, 7
    sw t1, 0(t0)
    li t0, 0x10801048
    li t1, 0x3f010100
    sw t1, 0(t0)
    li t0, 0x20585220
    li t1, 0x212
    sw t1, 0(t0)
    li t0, 0x1f
    csrw 0x347, t0
    la t0, handled
    sw zero, 0(t0)
    li s1, 0x20801000
    li t0, 0x50d83aa1
    sw t0, 0x18(s1)
    li t0, 4
    sw t0, 4(s1)
    li t0, 3
    sw t0, 8(s1)
    li t0, 0x96000000
    sw t0, 0(s1)
    li t0, 0x80000000
    sw t0, 0x2c(s1)
    sw zero, 0x18(s1)
    la s0, ready_text
    jal ra, puts
    csrsi mstatus, 8
wait:
    la t0, handled
    lw t1, 0(t0)
    bnez t1, done
    wfi
    j wait
done:
    csrr t0, 0xfb1
    bnez t0, failed
    la s0, pass_text
    jal ra, puts
halt:
    wfi
    j halt
wdt_irq:
    csrr t0, 0xfb1
    li t1, 0x3f000000
    bne t0, t1, failed
    lw t0, 0x28(s1)
    li t1, 0x80000000
    bne t0, t1, failed
    sw t1, 0x30(s1)
    la t0, handled
    li t1, 1
    sw t1, 0(t0)
    mret
.balign 64
failed:
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
.balign 64
vector_table:
.rept 18
    .word failed
.endr
    .word wdt_irq
.rept 29
    .word failed
.endr
ready_text: .asciz "S31 WDT READY\n"
pass_text: .asciz "S31 WDT IRQ PASS\n"
fail_text: .asciz "S31 WDT FAIL\n"
.section .bss,"aw",@nobits
.balign 4
handled: .space 4

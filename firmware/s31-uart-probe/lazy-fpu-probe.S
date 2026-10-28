/* Architectural FS-Off exception -> real guest handler -> same FLW retires.
 * UART input L leaves FS Off to test a bounded repeated-exception stop;
 * X causes an unmapped load,which diagnostic continuation must not hide.
 * This fixture tests RV32F behavior,not the private Plus SDK's lazy-FPU policy. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    la t0, handler
    ori t0, t0, 3
    csrw mtvec, t0
    li t0, 0x2038a000
    lw s4, 0(t0)
    li t1, 88
    bne s4, t1, fp_start
    lw t0, 0(zero)
fp_start:
    la s1, input_float
faulting_flw:
.option push
.option norvc
    flw fa0, 0(s1)
.option pop
    fadd.s fa1, fa0, fa0
    fmv.x.w t0, fa1
    li t1, 0x40400000
    bne t0, t1, fail
    li t0, 0x2f060004
    fsw fa1, 0(t0)
    la s0, pass_text
    jal ra, puts
wait:
    wfi
    j wait
.balign 64
handler:
    csrr t0, mcause
    andi t0, t0, 255
    li t1, 2
    bne t0, t1, fail
    csrr t0, mepc
    la t1, faulting_flw
    bne t0, t1, fail
    li t0, 0x2f060000
    lw t1, 0(t0)
    addi t1, t1, 1
    sw t1, 0(t0)
    li t1, 76
    beq s4, t1, unchanged
    li t0, 0x2000
    csrs mstatus, t0
unchanged:
    mret
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
.balign 4
input_float: .word 0x3fc00000
pass_text: .asciz "S31 GUEST LAZY FPU MRET PASS\n"
fail_text: .asciz "S31 GUEST LAZY FPU FAIL\n"

/* Real FS-Off memory operations and guest EXT_ILL checks across full/compressed
 * RV32F loads/stores. This is a diagnostic source image,not an official firmware
 * replacement. Handler changes its own FS and uses mret to retry each operation. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li t0, 0x3fc00000
    sw t0, 0(sp)
    la t0, handler
    ori t0, t0, 3
    csrw mtvec, t0
    la s0, input_float
    li s2, 0x6000
    csrc mstatus, s2
.option push
.option norvc
    flw ft0, 0(s0)
.option pop
    csrc mstatus, s2
.option push
.option norvc
    fsw ft0, 4(s0)
.option pop
    csrc mstatus, s2
    c.flw fs0, 0(s0)
    csrc mstatus, s2
    c.fsw fs0, 4(s0)
    csrc mstatus, s2
    c.flwsp fa0, 0(sp)
    csrc mstatus, s2
    c.fswsp fa0, 4(sp)
    lw t0, 4(s0)
    li t1, 0x3fc00000
    bne t0, t1, fail
    lw t0, 4(sp)
    bne t0, t1, fail
    li t0, 0x2f060000
    lw t0, 0(t0)
    li t1, 6
    bne t0, t1, fail
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
    csrrw t0, 0x7f0, zero
    li t1, 1
    bne t0, t1, fail
    li t0, 0x2f060000
    lw t1, 0(t0)
    addi t1, t1, 1
    sw t1, 0(t0)
    li t0, 0x2000
    csrs mstatus, t0
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
.section .data,"aw",@progbits
.balign 4
input_float: .word 0x3fc00000, 0
.section .rodata,"a",@progbits
pass_text: .asciz "S31 FPU MEMORY REASONS PASS\n"
fail_text: .asciz "S31 FPU MEMORY REASON FAIL\n"

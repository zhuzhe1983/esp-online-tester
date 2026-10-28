/* Fixed ESP-IDF6.1 S31 spi_mem_s_reg.h/mspi_periph.c: system MSPI2 source112.
 * Actual invalid MMU loads/stores raise both access exceptions and error IRQs.
 * Each guest exception handler advances its own mepc; the host only resumes. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f06f000
    li s0, 0x2f060000
    sw zero, 0(s0)
    li s1, 0x20502000
    li s3, 0
    li s4, 0
    li t0, 0x207041e8
    li t1, 0x80000000
    sw t1, 0(t0)
    li t0, 0x205851c0
    li t1, 0x211
    sw t1, 0(t0)
    li t0, 0x10801044
    li t1, 0x3f010100
    sw t1, 0(t0)
    li t0, 0x10800000
    li t1, 7
    sw t1, 0(t0)
    la t0, exception
    ori t0, t0, 3
    csrw mtvec, t0
    la t0, vector_table
    csrw 0x307, t0
    li t0, 0x1f
    csrw 0x347, t0
    li t0, 0x280
    sw t0, 0xc0(s1)
    csrsi mstatus, 8
    li s2, 0x80
    li t0, 0x50000000
.option push
.option norvc
    lw t1, 0(t0)                 /* real read address fault */
.option pop
    li t0, 1
    bne s3, t0, unexpected
    bne s4, t0, unexpected
    li s2, 0x200
    li t0, 0x50000000
    li t1, 0x12345678
.option push
.option norvc
    sw t1, 0(t0)                 /* real write address fault */
.option pop
    li t0, 2
    bne s3, t0, unexpected
    bne s4, t0, unexpected
    /* The guest repairs its mapping after both errors, then checks real data. */
    li t0, 0xc00
    sw t0, 0x37c(s1)
    li t0, 0x50000000
    li t1, 0x12345678
    sw t1, 0(t0)
    lw t2, 0(t0)
    bne t1, t2, unexpected
    lw t0, 0xc8(s1)
    bnez t0, unexpected
    sw s3, 0(s0)
    la s0, pass_msg
    call puts
    csrci mstatus, 8
1:  wfi
    j 1b
.balign 64
exception:
    csrr t0, mcause
    andi t0, t0, 0xff           /* CLIC mcause also carries MPIE/MPP state */
    li t1, 5
    beqz s4, 1f
    li t1, 7
1:  bne t0, t1, unexpected
    csrr t0, mtval
    li t1, 0x50000000
    bne t0, t1, unexpected
    addi s4, s4, 1
    csrr t0, mepc
    addi t0, t0, 4
    csrw mepc, t0
    mret
psram_irq:
    lw t0, 0xcc(s1)
    bne t0, s2, unexpected
    sw s2, 0xc8(s1)              /* RAW writes cannot clear or forge events */
    lw t0, 0xcc(s1)
    bne t0, s2, unexpected
    sw s2, 0xc4(s1)
    lw t0, 0xcc(s1)
    bnez t0, unexpected
    addi s3, s3, 1
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
    .word psram_irq
.rept 30
    .word unexpected
.endr
pass_msg: .asciz "S31 PSRAM REAL FAULT IRQ MRET PASS\n"
fail_msg: .asciz "S31 PSRAM IRQ FAIL\n"

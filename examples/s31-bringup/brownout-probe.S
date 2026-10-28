/* Fixed IDF6.1 S31 mode0 and target0x61 register5 threshold RMW.
 * Input is qualified comparator state plus detector clock edges,not voltage.
 * Flash/RF shutdown disabled here; those physical actions remain unsupported. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f06f000
    li s1, 0x20702000
    li s2, 0x2f060000
    sw zero, 0(s2)
    li t0, 0x2f070000
    lw s3, 0(t0)
    li t0, 0x2010f018
    li t1, 4
    sw t1, 0(t0)
    li t0, 0x2010f804
    li t1, 0x561
    sw t1, 0(t0)
    lw t1, 0(t0)
    srli t1, t1, 16
    andi t1, t1, 0xf8
    ori t1, t1, 5
    slli t1, t1, 16
    li t2, 0x01000561
    or t1, t1, t2
    sw t1, 0(t0)
    li t1, 0x561
    sw t1, 0(t0)
    lw t1, 0(t0)
    srli t1, t1, 16
    li t2, 0xfd
    bne t1, t2, unexpected
    lw t0, 0x1c(s1)
    andi t0, t0, -3
    sw t0, 0x1c(s1)              /* known software policy,mode1 off */
    sw zero, 4(s1)
    li t0, 0x2058523c            /* source143 */
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
    li t0, 0x20100200            /* enable,IRQ wait2,reset wait4 */
    li t1, 1
    bne s3, t1, 1f
    li t1, 0xc0000000           /* system reset mode */
    or t0, t0, t1
1:  li t1, 2
    bne s3, t1, 2f
    ori t0, t0, 0x80             /* unsupported physical RF action negative */
2:  sw t0, 0(s1)
    bnez s3, 3f
    li t0, 0x80000000
    sw t0, 0x28(s1)
3:  la s0, armed_msg
    call puts
    csrsi mstatus, 8
wait:
    lw t0, 0(s2)
    bnez t0, done
    wfi
    j wait
done:
    la s0, pass_msg
    call puts
    csrci mstatus, 8
1:  wfi
    j 1b
brownout_irq:
    lw t0, 0x24(s1)
    li t1, 0x80000000
    bne t0, t1, unexpected
    lw t0, 0x30(s1)
    bne t0, t1, unexpected
    sw t1, 0x20(s1)
    lw t0, 0x24(s1)
    bne t0, t1, unexpected
    sw t1, 0x2c(s1)
    lw t0, 0x24(s1)
    bnez t0, unexpected
    lw t0, 0x30(s1)
    bne t0, t1, unexpected
    sw t1, 0x3c(s1)
    sw zero, 0(s1)
    li t0, 1
    sw t0, 0(s2)
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
    .word brownout_irq
.rept 30
    .word unexpected
.endr
armed_msg: .asciz "S31 BROWNOUT MODE0 ARMED\n"
pass_msg: .asciz "S31 BROWNOUT REAL IRQ MRET PASS\n"
fail_msg: .asciz "S31 BROWNOUT FAIL\n"

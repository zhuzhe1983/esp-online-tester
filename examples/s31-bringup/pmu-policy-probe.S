/* Fixed IDF6.1 S31 PMU mode policy/WT/force-control protocol. No physical
 * regulator voltage,clock-lock or sleep-transition success is claimed. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20704000
    li s2, 0x2f070000
    lw s3, 0(s2)
    li t0, 1
    bne s3, t0, 1f
    li t0, 0x20000000
    sw t0, 0(s1)              /* unsupported active CPU powerdown */
1:  li t0, 3
    bne s3, t0, 2f
    li t0, 1
    sw t0, 0x120(s1)          /* unsupported asserted TOP reset */
2:  li t0, 0x20000000
    sw t0, 0x80(s1)           /* stored sleep CPU-off policy */
    lw t1, 0x80(s1)
    bne t0, t1, fail
    lw t0, 0(s1)
    bnez t0, fail             /* active policy must stay powered */
    sw zero, 0x88(s1)         /* sleep APB disabled,not active APB */
    li t0, 2
    bne s3, t0, 3f
    sw zero, 8(s1)            /* unsupported active APB apply */
3:  li t0, 0x80000000
    sw t0, 0x100(s1)
    lw t0, 0x100(s1)
    bnez t0, fail             /* WT command reads zero */
    lw t0, 0x34(s1)
    li t1, 0xffffc00f
    and t0, t0, t1
    li t1, 0x38d8             /* requested LP13/HP28 and DBIAS_INIT */
    or t0, t0, t1
    sw t0, 0x34(s1)
    lw t0, 0x34(s1)
    li t1, 0x3ff8
    and t0, t0, t1
    li t1, 0x3180             /* documented RO reset LP24/HP24,WT0 */
    bne t0, t1, fail
    lw t0, 0x120(s1)
    li t1, 28
    bne t0, t1, fail
    sw zero, 0x120(s1)        /* release force-on/no-reset/no-isolate */
    lw t0, 0x120(s1)
    bnez t0, fail
    sw zero, 0x13c(s1)        /* release RAM force controls,no reset */
    lw t0, 0(s2)
    bne t0, s3, fail
    la s0, passed
    jal ra, puts
wait:
    wfi
    j wait
fail:
    la s0, failed
    jal ra, puts
    ebreak
puts:
    li t3, 0x2038a000
4:  lbu t4, 0(s0)
    beqz t4, 5f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 4b
5:  ret
.section .rodata,"a",@progbits
passed: .asciz "S31 PMU POLICY PASS\n"
failed: .asciz "S31 PMU POLICY FAIL\n"

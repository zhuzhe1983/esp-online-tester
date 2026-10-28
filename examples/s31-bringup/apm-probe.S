/* S31 APM testbench. The host explicitly wires HP_MEM path2 to the 128-byte
 * test page; this is not a claim about the production S31 master topology.
 * Every case runs the same original guest writes/reads/instruction fetch. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s0, 0x2f060000
    li t0, 0x02a00513       /* addi a0,zero,42 */
    sw t0, 0(s0)
    li t0, 0x00008067       /* jalr zero,ra,0 */
    sw t0, 4(s0)
    li t0, 0x12345678
    sw t0, 8(s0)
    li s1, 0x20504800
    li s2, 0x2f070000
    lw s3, 0(s2)            /* requested TEE permissions */
    lw t0, 4(s2)            /* bus-error response */
    sw t0, 0x12c(s1)
    lw t0, 8(s2)            /* path function enable mask */
    sw t0, 0xc4(s1)
    sw s3, 12(s1)
    lw t0, 12(s1)
    bne t0, s3, fail
    li t0, 0xdeadbeef
    sw t0, 8(s0)
    lw t1, 8(s0)
    bne t0, t1, fail
    jalr ra, s0, 0
    li t0, 42
    bne a0, t0, fail
    li t0, 0x205048c4
    li t1, 0x3f
    sw t1, 0(t0)
    li t0, 0x2050480c
    li t1, 0x7000
    sw t1, 0(t0)
    la a1, success
    li a2, 0x2038a000
1:  lbu t0, 0(a1)
    beqz t0, done
    sw t0, 0(a2)
    addi a1, a1, 1
    j 1b
done:
    wfi
    j done
fail:
    ebreak
.section .rodata
success: .asciz "S31 APM RWX PASS\n"

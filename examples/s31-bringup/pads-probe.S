/* Fixed IDF v6.1 S31 GPIO/IO_MUX/PMU maps. Software GPIO10 (HP) and GPIO2 (LP).
 * Host samples the real held output, changes GPIO2 input, then supplies UART 'A'.
 * The guest verifies held output/configuration plus live input, releases the two
 * PMU domains separately, and checks GPIO_IN before printing PASS.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20583000
    li s2, 0x20582000
    li s3, 0x2070410c
    li t0, 0x1200                   /* GPIO function1, input enabled */
    sw t0, 8(s2)                    /* GPIO2 */
    sw t0, 40(s2)                   /* GPIO10 */
    li t0, 0x400
    sw t0, 0x38(s1)                 /* GPIO10 output enable, initially low */
    li t0, 0x50000000
    sw t0, 0(s3)                    /* Hold LP and HP */
    li t0, 0x400
    sw t0, 8(s1)                    /* Register goes high, held pad stays low */
    li t0, 0x1000
    sw t0, 8(s2)                    /* Disable input in register, held input remains enabled */
    la s0, ready
    jal ra, puts
    li t3, 0x2038a000
1:  lw t0, 0x1c(t3)
    andi t0, t0, 255
    beqz t0, 1b
    lw t0, 0(t3)
    li t1, 65
    bne t0, t1, fail
    lw t0, 0x64(s1)
    li t1, 4
    bne t0, t1, fail                /* LP GPIO2 live-high, HP GPIO10 held-low */
    li t0, 0x80000000
    sw t0, 0(s3)
    lw t0, 0x64(s1)
    li t1, 0x404
    bne t0, t1, fail
    li t0, 0x20000000
    sw t0, 0(s3)
    lw t0, 0x64(s1)
    li t1, 0x400
    bne t0, t1, fail
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
ready: .asciz "S31 PAD HOLD READY\n"
pass: .asciz "S31 PAD HOLD INPUT PASS\n"
failed: .asciz "S31 PAD FAIL\n"

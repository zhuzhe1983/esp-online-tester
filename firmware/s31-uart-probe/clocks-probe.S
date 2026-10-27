/* Fixed 20260528 S31 ROM ets_clk_get_cpu_freq and v6.1 UART reset controls. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07cfb0
    li s1, 0x20587000
    li s2, 0x2038a000
    li t0, 0x2f806ba2
    jalr ra, t0
    li t1, 40000000
    bne a0, t1, fail
    li t0, 1
    sw t0, 4(s1)
    sw t0, 0x14(s1)
    lw t0, 0x14(s1)
    bnez t0, fail
    li t0, 0x2f806ba2
    jalr ra, t0
    li t1, 20000000
    bne a0, t1, fail
    li t0, 347
    sw t0, 0x14(s2)
    la s0, ready
    jal ra, puts
1:  lw t0, 0x1c(s2)
    andi t0, t0, 255
    beqz t0, 1b
    li t0, 1
    sw t0, 0xc(s2)
    li t0, 0x87
    sw t0, 0x88(s1)           /* core reset clears pending RX, preserves configuration */
    li t0, 0x83
    sw t0, 0x88(s1)
    lw t0, 0x1c(s2)
    andi t0, t0, 255
    bnez t0, fail
    lw t0, 0xc(s2)
    li t1, 1
    bne t0, t1, fail
    li t0, 0x8b
    sw t0, 0x88(s1)           /* APB module reset restores register state */
    li t0, 0x83
    sw t0, 0x88(s1)
    lw t0, 0xc(s2)
    bnez t0, fail
    lw t0, 0x14(s2)
    li t1, 694
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
3:  lbu t4, 0(s0)
    beqz t4, 4f
    sw t4, 0(s2)
    addi s0, s0, 1
    j 3b
4:  ret
.section .rodata,"a",@progbits
ready: .asciz "S31 ROM CLOCK20 READY\n"
pass: .asciz "S31 UART RESET PASS\n"
failed: .asciz "S31 CLOCK FAIL\n"

/* Source-built S31 dual-hart fixture. CPU1 reaches secondary through the real
 * 20260528 ROM reset/main loop and ets_set_appcpu_boot_addr, never a host PC jump.
 * Shared SRAM atomics, independent CLIC state, guest stall/resume and reset. */
.option norvc
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f06f000
    li s0, 0x2f060000
    sw zero, 0(s0)                 /* ready */
    sw zero, 4(s0)                 /* start */
    sw zero, 8(s0)                 /* shared counter */
    sw zero, 12(s0)                /* done */
    sw zero, 16(s0)                /* core0 SC failures */
    sw zero, 20(s0)                /* core1 SC failures */
    sw zero, 24(s0)                /* restart handshake */
    li t0, 0x10801044
    li t1, 0x3f000100
    sw t1, 0(t0)
    la a0, secondary
    li t0, 0x2f8000a8             /* ROM ets_set_appcpu_boot_addr */
    jalr ra, t0, 0
    li t0, 0x207041ec
    li t1, 0x00ff0000
    sw t1, 0(t0)
    li t0, 0x20587020
    li t1, 7
    sw t1, 0(t0)
    li t1, 3
    sw t1, 0(t0)
wait_ready:
    lw t0, 0(s0)
    beqz t0, wait_ready
    li t0, 0x10801044
    lw t1, 0(t0)
    li t2, 0x3fc00100
    bne t1, t2, fail
    li t0, 1
    sw t0, 4(s0)
    li a1, 200
    li a2, 0
    addi a3, s0, 8
    call increment
    sw a2, 16(s0)
wait_done:
    lw t0, 12(s0)
    beqz t0, wait_done
    lw t0, 8(s0)
    li t1, 400
    bne t0, t1, fail
    /* CPU1 now spins on counter24. Freeze it, then verify no writes occur. */
    li t0, 0x207041ec
    li t1, 0x00860000
    sw t1, 0(t0)
    li t0, 0x20586064
    lw t1, 0(t0)
    li t2, 2
    bne t1, t2, fail
    lw t2, 24(s0)
    li t3, 20
pause_check:
    addi t3, t3, -1
    bnez t3, pause_check
    lw t1, 24(s0)
    bne t1, t2, fail
    li t0, 0x207041ec
    li t1, 0x00ff0000
    sw t1, 0(t0)
wait_resume:
    lw t1, 24(s0)
    beq t1, t2, wait_resume
    /* Reset CPU1 back to real ROM; a different guest boot address must run. */
    li t0, 0x20587020
    li t1, 7
    sw t1, 0(t0)
    la a0, restarted
    li t0, 0x2f8000a8
    jalr ra, t0, 0
    sw zero, 24(s0)
    li t0, 0x20587020
    li t1, 3
    sw t1, 0(t0)
wait_restart:
    lw t1, 24(s0)
    li t2, 0x5a31
    bne t1, t2, wait_restart
    la a0, pass_msg
    call puts
    wfi
    j .
increment:
    lr.w t0, (a3)
    nop
    nop
    nop
    addi t0, t0, 1
    sc.w t1, t0, (a3)
    beqz t1, increment_ok
    addi a2, a2, 1
    j increment
increment_ok:
    addi a1, a1, -1
    bnez a1, increment
    ret
secondary:
    li sp, 0x2f06e000
    csrr t0, mhartid
    li t1, 1
    bne t0, t1, fail
    li s0, 0x2f060000
    li t0, 0x10801044
    lw t1, 0(t0)
    li t2, 0x1fc00000
    bne t1, t2, fail
    li t1, 0x7f000100
    sw t1, 0(t0)
    li t0, 1
    sw t0, 0(s0)
wait_start:
    lw t0, 4(s0)
    beqz t0, wait_start
    li a1, 200
    li a2, 0
    addi a3, s0, 8
    call increment
    sw a2, 20(s0)
    li t0, 1
    sw t0, 12(s0)
heartbeat:
    lw t0, 24(s0)
    addi t0, t0, 1
    sw t0, 24(s0)
    j heartbeat
restarted:
    csrr t0, mhartid
    li t1, 1
    bne t0, t1, fail
    li s0, 0x2f060000
    li t0, 0x5a31
    sw t0, 24(s0)
    wfi
    j .
puts:
    li t0, 0x2038a000
1:  lbu t1, 0(a0)
    beqz t1, 2f
    sw t1, 0(t0)
    addi a0, a0, 1
    j 1b
2:  ret
fail:
    la a0, fail_msg
    call puts
    ebreak
pass_msg: .asciz "S31 MULTICORE ROM ATOMIC RESET PASS\n"
fail_msg: .asciz "S31 MULTICORE FAIL\n"

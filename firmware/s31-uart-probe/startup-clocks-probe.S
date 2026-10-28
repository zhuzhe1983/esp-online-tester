/* Fixed S31 IDF6.1 startup clock requests, existing SHA gate, and actual PMS denial.
 * No simulated completion from host: all assertions, mailbox and UART are guest writes.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li t0, 0x2f070000
    lw s4, 0(t0)
    li s1, 0x20587000
    lw t0, 0xa8(s1)           /* ETM core clock initially requested */
    andi t1, t0, 64
    beqz t1, fail
    andi t0, t0, -65
    sw t0, 0xa8(s1)
    lw t0, 0xa8(s1)
    andi t0, t0, 64
    bnez t0, fail
    sw zero, 0x24(s1)         /* independent trace/DMAs */
    sw zero, 0x78(s1)
    sw zero, 0x7c(s1)
    sw zero, 0x80(s1)
    sw zero, 0x6c(s1)
    sw zero, 0x70(s1)
    sw zero, 0x1b0(s1)
    sw zero, 0x1d0(s1)
    li t0, 9                 /* disabled LCDCAM requested source1/div2 */
    sw t0, 0x154(s1)
    li t1, 0x205043f8
    sw zero, 0(t1)           /* automatic TEE clock gating,not permissions */
    lw t0, 0(t1)
    bnez t0, fail
    li t1, 0x20706bf8
    sw zero, 0(t1)
    li t1, 0x20359030
    lw t0, 0(t1)
    li t2, 0xff7fffff
    and t0, t0, t2
    sw t0, 0(t1)             /* USB OTG PHY/reset state retained */
    lw t0, 0(t1)
    li t2, 0xe0000000
    bne t0, t2, fail
    li t1, 0x207133f8
    sw zero, 0(t1)
    li t1, 0x20710000
    sw zero, 0x18(t1)
    sw zero, 0x2c(t1)
    sw zero, 0x50(t1)
    li t0, 2
    beq s4, t0, unmapped_etm
    li t0, 3
    beq s4, t0, unimplemented_lcd
    li t0, 4
    beq s4, t0, deny_tee
    li t0, 5
    beq s4, t0, deny_lp_clock
    li t0, 6
    beq s4, t0, deny_cnnt
    li s2, 0x20509000
    li t0, 2                 /* SHA256 of the real padded abc block */
    sw t0, 0(s2)
    li t0, 0x80636261
    sw t0, 0x80(s2)
    li t0, 0x18000000
    sw t0, 0xbc(s2)
    li t0, 1
    sw t0, 0x10(s2)
    lw t1, 0x40(s2)
    li t0, 0xbf1678ba
    bne t0, t1, fail
    lw s3, 0x58(s1)
    andi t0, s3, -65         /* genuine existing SHA core gate */
    sw t0, 0x58(s1)
    li t0, 1
    beq s4, t0, gated_sha
    sw s3, 0x58(s1)
    lw t1, 0x40(s2)
    li t0, 0xbf1678ba
    bne t0, t1, fail          /* existing digest survived gating */
    li t0, 0x2f060000
    li t1, 1
    sw t1, 0(t0)
    la s0, passed
    jal ra, puts
1:  wfi
    j 1b
gated_sha:
    lw t0, 0x40(s2)          /* must fault,not fabricate preserved data */
    j fail
unmapped_etm:
    li t1, 0x20393000
    lw t0, 0(t1)
    j fail
unimplemented_lcd:
    li t0, 13                /* CLK_EN remains unsupported */
    sw t0, 0x154(s1)
    j fail
deny_tee:
    li t1, 0x20505000
    li t2, 0x20505ff0
    li t0, 1
    sw t0, 0(t2)             /* strict bus error reply */
    li t0, 16
    sw t0, 0x58(t1)          /* deny TEE reads,retain writes */
    li t1, 0x205043f8
    lw t0, 0(t1)
    j fail
deny_lp_clock:
    li t1, 0x20706000
    li t2, 0x207067f0
    li t0, 1
    sw t0, 0(t2)
    li t0, 16
    sw t0, 0x38(t1)
    li t1, 0x20710018
    lw t0, 0(t1)
    j fail
deny_cnnt:
    li t1, 0x20505000
    li t2, 0x20505ff0
    li t0, 1
    sw t0, 0(t2)
    li t0, 16
    sw t0, 0x4c(t1)
    li t1, 0x20359030
    lw t0, 0(t1)
    j fail
fail:
    la s0, failed
    jal ra, puts
    ebreak
puts:
    li t3, 0x2038a000
2:  lbu t4, 0(s0)
    beqz t4, 3f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 2b
3:  ret
.section .rodata,"a",@progbits
passed: .asciz "S31 STARTUP CLOCKS SHA PRESERVE PASS\n"
failed: .asciz "S31 STARTUP CLOCKS FAIL\n"

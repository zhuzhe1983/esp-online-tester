/* Fixed IDF6.1 S31 MPLL and PSRAM source selectors; functional clock protocol,
 * not silicon voltage/settling time. Execute code only after real memory writes. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li t0, 0x2f070000
    lw s3, 0(t0)
    li s1, 0x20704000
    li t0, 0x00400000
    sw t0, 0xf4(s1)            /* MPLL global ICG */
    li t0, 0x44000000
    sw t0, 0xf4(s1)            /* MPLL circuit and I2C power */
    sw t0, 0x1c(s1)            /* active mode power policy */
    li t0, 0x20589000
    li t1, 0xfffe0000
    sw t1, 0(t0)               /* root MPLL clock gate */
    li t0, 2
    beq s3, t0, 1f
    li t0, 0x80000000
    sw t0, 0x1e8(s1)           /* PSRAM PHY power */
1:  li t0, 1
    beq s3, t0, 2f
    li t0, 0x2010f018
    li t1, 4
    sw t1, 0(t0)               /* analog master clock */
2:  li s2, 0x20587000
    li t0, 5
    bne s3, t0, 3f
    li t0, 0x300
    sw t0, 0x174(s2)           /* RO done write + stopped,not calibration */
    j map
3:  sw zero, 0x174(s2)         /* explicit calibration start */
    li t0, 0x20701054
    li t1, 0x99                /* (19+1)*40/2 = 400MHz */
    sw t1, 0(t0)
    li t2, 512
poll:
    lw t0, 0x174(s2)
    andi t1, t0, 256
    bnez t1, calibrated
    addi t2, t2, -1
    bnez t2, poll
    j fail
calibrated:
    ori t0, t0, 512
    sw t0, 0x174(s2)           /* stop completed calibration,keep result */
map:
    li t0, 0x3a1               /* MPLL/2 -> 200MHz PSRAM core */
    sw t0, 0x68(s2)
    li t0, 0x20502000
    li t1, 0xc00
    sw t1, 0x37c(t0)
    li t0, 0x50000000
    li t1, 0x02a00513          /* addi a0,zero,42 */
    sw t1, 0(t0)
    li t1, 0x00008067          /* ret */
    sw t1, 4(t0)
    li t1, 3
    bne s3, t1, 4f
    li t1, 0x321               /* PLL path gated off before fetch */
    sw t1, 0x68(s2)
4:  li t1, 4
    bne s3, t1, 5f
    li t1, 0x20701054
    li t2, 0xc1                /* new divider invalidates old calibration */
    sw t2, 0(t1)
5:  jalr ra, t0, 0
    li t1, 42
    bne a0, t1, fail
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
6:  lbu t4, 0(s0)
    beqz t4, 7f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 6b
7:  ret
.section .rodata,"a",@progbits
passed: .asciz "S31 MPLL PSRAM EXECUTE PASS\n"
failed: .asciz "S31 MPLL FAIL\n"

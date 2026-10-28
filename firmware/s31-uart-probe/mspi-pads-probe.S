/* S31 fixed IDF6.1 dedicated PSRAM pin configuration and functional HOLD.
 * No physical delay,drive strength or signal-integrity validation is implied. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li t0, 0x2f070000
    lw s3, 0(t0)
    li t0, 0x207041e8
    li t1, 0x80000000
    sw t1, 0(t0)
    li t0, 0x2050237c
    li t1, 0xc00
    sw t1, 0(t0)
    li s1, 0x20584000
    lw t0, 0(s1)
    li t1, 1
    bne t0, t1, fail
    sw zero, 0(s1)             /* auto-gating policy off,not a clock-off request */
    lw t0, 0x44(s1)
    li t1, 0x4000
    bne t0, t1, fail
    li t0, 0xc000
    sw t0, 0x44(s1)            /* hold CS configuration */
    li t0, 0xd234
    sw t0, 0x44(s1)
    lw t1, 0x44(s1)
    bne t0, t1, fail
    li t0, 1
    beq s3, t0, write_code     /* held CS must deny the external store */
    li t0, 0x5234
    sw t0, 0x44(s1)            /* release and apply configured fields */
    li t0, 0x20001
    sw t0, 0x3c(s1)            /* request DQS power; no RAM identity implied */
write_code:
    li t0, 0x50000000
    li t1, 0x02a00513
    sw t1, 0(t0)
    li t1, 0x00008067
    sw t1, 4(t0)
    li t1, 3
    bne s3, t1, 3f
    li t1, 0xc000
    sw t1, 0x1c(s1)            /* held data pin must deny real data read */
3:  lw t2, 0(t0)
    li t1, 0x02a00513
    bne t1, t2, fail
    li t1, 2
    bne s3, t1, execute
    li t1, 0x420001
    sw t1, 0x3c(s1)            /* held DQS must deny the instruction fetch */
execute:
    jalr ra, t0, 0
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
1:  lbu t4, 0(s0)
    beqz t4, 2f
    sw t4, 0(t3)
    addi s0, s0, 1
    j 1b
2:  ret
.section .rodata,"a",@progbits
passed: .asciz "S31 MSPI PADS EXECUTE PASS\n"
failed: .asciz "S31 MSPI PADS FAIL\n"

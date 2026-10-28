/* Fixed S31 IDF6.1 MSPI2/3 and AP mode-register/data transactions.
 * External MR responses/backing are explicit host inputs; no identified product die. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li t0, 0x2f070000
    lw s3, 0(t0)
    li t0, 3
    beq s3, t0, dqs
    li t0, 0x207041e8
    li t1, 0x80000000
    sw t1, 0(t0)
dqs:
    li t0, 2
    beq s3, t0, configure
    li t0, 0x2058403c
    li t1, 0x20001
    sw t1, 0(t0)
configure:
    li s1, 0x20503000
    li t0, 0x2ca27c
    sw t0, 8(s1)
    li t0, 0x21
    sw t0, 0xd4(s1)
    li t0, 1
    sw t0, 0x34(s1)
    li t0, 0xf0000000
    sw t0, 0x18(s1)
    li t0, 0x7c00000b
    sw t0, 0x1c(s1)
    li t0, 0xf0004040
    li t1, 8
    bne s3, t1, 1f
    li t0, 0xf000beef
1:  sw t0, 0x20(s1)
    li t0, 15
    sw t0, 0x28(s1)
    li t0, 4
    li t1, 7
    bne s3, t1, 2f
    li t0, 6
2:  sw t0, 4(s1)
    li t0, 4
    bne s3, t0, read_mode
    li t0, 0x20587068
    li t1, 0x81
    sw t1, 0(t0)
read_mode:
    li t0, 0x40000
    sw t0, 0(s1)
    lw t0, 0x58(s1)
    li t1, 0x0520
    bne t0, t1, fail
    li t0, 0xc8000000
    sw t0, 0x18(s1)
    li t0, 0xf000c0c0
    sw t0, 0x20(s1)
    li t0, 8
    sw t0, 4(s1)
    li t0, 15
    sw t0, 0x24(s1)
    li t0, 0xff0b
    sw t0, 0x58(s1)            /* second byte is don't-care,not MR0 write */
    li t0, 0x40000
    sw t0, 0(s1)
    li t0, 0xf0000000
    sw t0, 0x18(s1)
    li t0, 0xf0004040
    sw t0, 0x20(s1)
    li t0, 0x40000
    sw t0, 0(s1)
    lw t0, 0x58(s1)
    li t1, 0x0c0b
    bne t0, t1, fail
    li t0, 0xe8000000
    sw t0, 0x18(s1)
    li t0, 0xf0008080
    sw t0, 0x20(s1)
    sw zero, 4(s1)
    li t0, 63
    sw t0, 0x24(s1)
    sw t0, 0x28(s1)
    li t0, 0x02a00513
    sw t0, 0x58(s1)
    li t0, 0x00008067
    sw t0, 0x5c(s1)
    li t0, 0x40000
    sw t0, 0(s1)
    sw zero, 0x58(s1)
    sw zero, 0x5c(s1)
    li t0, 0xf0000000
    sw t0, 0x18(s1)
    sw t0, 0x20(s1)
    li t0, 0x40000
    sw t0, 0(s1)
    lw t0, 0x58(s1)
    li t1, 0x02a00513
    bne t0, t1, fail
    lw t0, 0x5c(s1)
    li t1, 0x8067
    bne t0, t1, fail
    li s1, 0x20502000
    li t0, 0xc00
    sw t0, 0x37c(s1)
    li t0, 1
    li t1, 5
    bne s3, t1, 3f
    li t0, 0x80000001
3:  sw t0, 0x3c(s1)
    li t0, 0x37c021
    sw t0, 0x40(s1)
    li t0, 0x3c0000
    sw t0, 0x44(s1)
    li t0, 0xf0000000
    sw t0, 0x48(s1)
    li t0, 0xf0008080
    sw t0, 0x4c(s1)
    li t0, 0x3021
    sw t0, 0xd8(s1)
    li t0, 6
    bne s3, t0, execute
    li t0, 0x2058403c
    li t1, 0x20000
    sw t1, 0(t0)
execute:
    li t0, 0x50000000
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
passed: .asciz "S31 PSRAM SERIAL EXECUTE PASS\n"
failed: .asciz "S31 PSRAM SERIAL FAIL\n"

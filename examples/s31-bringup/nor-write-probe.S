/* S31 IDF6.1 SPI1 command/length registers + ROM20260528 SE|PE.
 * Winbond W25Q128JV: WEL,4KiB erase,page program,1-to-0 programming.
 * Guest erases/programs independent Flash input,reads it through actual MMU,
 * calls the programmed instructions,then verifies a second FF write cannot
 * restore zero bits. Mode1 omits erase WREN;mode2 omits program WREN. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li t0, 0x2f070000
    lw s3, 0(t0)
    li t0, 0x20500000
    li t1, 0x40000001
    sw t1, 0x3c(t0)
    sw zero, 0x380(t0)
    li t1, 0x1000
    sw t1, 0x37c(t0)
    li s1, 0x20501000
    li s2, 0x40001080
    li t0, 0x1000
    sw t0, 4(s1)
    li t0, 1
    beq s3, t0, erase
    li t0, 0x40000000
    sw t0, 0(s1)
erase:
    li t0, 0x01020000
    sw t0, 0(s1)
    lw t0, 0(s2)
    li t1, -1
    bne t0, t1, fail
    li t0, 0x40000000
    lw t0, 0(t0)
    li t1, 0xa5a5a5a5
    bne t0, t1, fail
    li t0, 0xc8000000
    sw t0, 0x18(s1)
    li t0, 0x70000002
    sw t0, 0x20(s1)
    li t0, 63
    sw t0, 0x24(s1)
    li t0, 0x1080
    sw t0, 4(s1)
    li t0, 0x02a00513          /* addi a0,zero,42 */
    sw t0, 0x58(s1)
    li t0, 0x00008067          /* jalr zero,ra,0 */
    sw t0, 0x5c(s1)
    li t0, 2
    beq s3, t0, program
    li t0, 0x40000000
    sw t0, 0(s1)
program:
    li t0, 0x00060000
    sw t0, 0(s1)
    lw t0, 0(s2)
    li t1, 0x02a00513
    bne t0, t1, fail
    jalr ra, s2, 0             /* actual instruction fetch of programmed Flash */
    li t0, 42
    bne a0, t0, fail
    li t0, -1
    sw t0, 0x58(s1)
    sw t0, 0x5c(s1)
    li t0, 0x40000000
    sw t0, 0(s1)
    li t0, 0x00060000
    sw t0, 0(s1)
    lw t0, 0(s2)
    li t1, 0x02a00513
    bne t0, t1, fail
    lw t0, 4(s2)
    li t1, 0x00008067
    bne t0, t1, fail
    li t0, 0x08000000
    sw t0, 0(s1)
    lw t0, 0x2c(s1)
    andi t0, t0, 3
    bnez t0, fail
    la s0, pass_text
    jal ra, puts
wait:
    wfi
    j wait
fail:
    la s0, fail_text
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
pass_text: .asciz "S31 NOR ERASE PROGRAM EXECUTE PASS\n"
fail_text: .asciz "S31 NOR WRITE FAIL\n"

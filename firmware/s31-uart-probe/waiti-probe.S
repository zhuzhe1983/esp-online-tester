/* Fixed IDF6.1 FLASH_WAITI controls,actual WEL/SE/PP and MMU code fetch.
 * Mode0: auto RDSR + timing dummy;1: wait disabled;2: wrong command;
 * 3: address phase;4: 16-bit command;5: actual suspend trigger.
 * Negative modes must trap before any Flash mutation; no guest recovery hack. */
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
    lw t0, 0x98(s1)
    li t1, 0x50001
    bne t0, t1, fail
    lw t0, 0x9c(s1)
    ori t0, t0, 12
    sw t0, 0x9c(s1)
    lw t1, 0x9c(s1)
    bne t0, t1, fail
    li t1, 5
    bne s3, t1, config
    ori t0, t0, 2             /* actual unsupported suspend trigger */
    sw t0, 0x9c(s1)
config:
    li t0, 0x50803           /* ordinary RDSR with retained timing dummy */
    beqz s3, configured
    li t1, 1
    li t0, 0xabcdfe1e        /* disabled wait: no auto command is sent */
    beq s3, t1, configured
    li t1, 2
    li t0, 0x350001          /* wrong status command */
    beq s3, t1, configured
    li t1, 3
    li t0, 0x50005           /* unsupported address phase */
    beq s3, t1, configured
    li t0, 0x50201           /* unsupported 16-bit command */
configured:
    sw t0, 0x98(s1)
    lw t1, 0x98(s1)
    bne t0, t1, fail
    li t0, 0x1000
    sw t0, 4(s1)
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
pass_text: .asciz "S31 FLASH WAITI ERASE PROGRAM EXECUTE PASS\n"
fail_text: .asciz "S31 FLASH WAITI FAIL\n"

/* S31 IDF6.1 GPIO36 function0 GPIO alias; GPIO32 function0 Flash D.
 * Tests actual guest bank reads with explicit unresolved pad voltage input.
 * This is a diagnostic source fixture,not a Flash/UART waveform model. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s1, 0x20583000
    li s2, 0x20582000
    li t0, 0x200
    sw t0, 144(s2)               /* GPIO36 function0,input */
    sw t0, 128(s2)               /* GPIO32 Flash D,input */
    li t0, 16
    sw t0, 0x44(s1)
    sw t0, 0x14(s1)              /* real GPIO36 output high */
    lw t0, 0x68(s1)
    li t1, 16
    bne t0, t1, fail
    la s0, ready
    jal ra, puts
    li t3, 0x2038a000
1:  lw t0, 0x1c(t3)
    andi t0, t0, 255
    beqz t0, 1b
    lw t0, 0(t3)
    li t1, 65
    bne t0, t1, fail
    lw t0, 0x68(s1)
    li t1, 17
    bne t0, t1, fail
    la s0, passed
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
ready: .asciz "S31 GPIO SAMPLE READY\n"
passed: .asciz "S31 GPIO SAMPLE CHANGED PASS\n"
failed: .asciz "S31 GPIO SAMPLE FAIL\n"

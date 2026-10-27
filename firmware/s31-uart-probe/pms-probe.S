/* S31 v6.1 PMS fixture: host mailbox words are UART permission and bus-error enable.
 * The same guest sends A, configures and locks UART permission, attempts unlock,
 * reads real UART status then sends B. Denial must stop before its side effect. */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li s0, 0x2038a000
    li t0, 65
    sw t0, 0(s0)
    li s1, 0x2058a000
    li s2, 0x2f070000
    lw s3, 0(s2)
    lw t0, 4(s2)
    li t1, 0x2058aff0
    sw t0, 0(t1)
    sw s3, 0x28(s1)
    li t0, 0xff
    sw t0, 0x28(s1)
    lw t0, 0x28(s1)
    bne t0, s3, fail
    lw t0, 0x1c(s0)
    li t0, 66
    sw t0, 0(s0)
1:  wfi
    j 1b
fail:
    ebreak

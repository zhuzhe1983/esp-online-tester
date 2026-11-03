/* C5 MP independent SRAM fixture. Addresses/fields: ESP-IDF v5.5.4@73550728.
 * UART0 source47 -> CLIC ID17, standard CSR threshold; GPIO28 active-low blue LED.
 * This is a local protocol fixture, not Stamp factory or UIFlow firmware. */
.section .text
.global _start
_start:
    la sp, stack_top
    li t0, 0x60090074
    li t1, 0x1000
    sw t1, 0(t0)
    li t0, 0x60091b44
    li t1, 256
    sw t1, 0(t0)
    li t0, 0x60091008
    li t1, 0x10000000
    sw t1, 0(t0)
    li t0, 0x60091038
    sw t1, 0(t0)
    la t0, handler
    ori t0, t0, 3
    csrw mtvec, t0
    csrw 0x347, zero
    li t0, 0x20800000
    li t1, 3
    sw t1, 0(t0)
    li t0, 0x20801044
    li t1, 0x3fc00100
    sw t1, 0(t0)
    li t0, 0x600100bc
    li t1, 17
    sw t1, 0(t0)
    li t0, 0x60000024
    li t1, 1
    sw t1, 0(t0)
    li t0, 0x6000000c
    sw t1, 0(t0)
    la a0, ready
    jal puts
    csrsi mstatus, 8
wait:
    wfi
    j wait
.balign 64
handler:
    addi sp, sp, -32
    sw t0, 0(sp)
    sw t1, 4(sp)
    sw t2, 8(sp)
    li t0, 0x60000000
    lw t2, 0(t0)
    sw t2, 0(t0)
    li t1, 82
    beq t2, t1, rmt
    li t1, 87
    beq t2, t1, wrong_pin
    li t1, 49
    beq t2, t1, on
    li t1, 48
    bne t2, t1, ack
    li t0, 0x60091008
    j drive
on:
    li t0, 0x6009100c
drive:
    li t1, 0x10000000
    sw t1, 0(t0)
    j ack
wrong_pin:
    li t0, 0x60090070
    li t1, 0x1000
    sw t1, 0(t0)
    li t0, 0x60091b40
    li t1, 256
    sw t1, 0(t0)
    li t0, 0x60091038
    li t1, 0x08000000
    sw t1, 0(t0)
    li t0, 0x6009100c
    sw t1, 0(t0)
    j ack
rmt:
    li t0, 0x60090018
    li t1, 0x1000
    sw t1, 0(t0)
    li t0, 0x60091ae8
    li t1, 64
    sw t1, 0(t0)
    li t0, 0x60006400
    li t1, 0x00148028
    sw t1, 0(t0)
    li t1, 0x00288014
    sw t1, 4(t0)
    sw zero, 8(t0)
    li t0, 0x60006010
    li t1, 0x00010101
    sw t1, 0(t0)
    li t0, 0x60006038
1:  lw t1, 0(t0)
    andi t1, t1, 1
    beqz t1, 1b
ack:
    li t0, 0x60000010
    li t1, -1
    sw t1, 0(t0)
    lw t0, 0(sp)
    lw t1, 4(sp)
    lw t2, 8(sp)
    addi sp, sp, 32
    mret
puts:
    li t0, 0x60000000
1:  lbu t1, 0(a0)
    beqz t1, 2f
    sw t1, 0(t0)
    addi a0, a0, 1
    j 1b
2:  ret
.section .rodata
ready: .asciz "C5 UART IRQ READY\n"
.section .bss
.balign 16
    .space 1024
stack_top:

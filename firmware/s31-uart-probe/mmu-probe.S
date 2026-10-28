/* S31 IDF v6.1 spi_mem_c/s_reg.h + ext_mem_defs.h + hal/mmu_ll.h.
 * Debugger SRAM entry configures real MMU ports, calls separately loaded flash code,
 * remaps a PSRAM virtual page and checks the physical contents survive the remap.
 * No ROM, bootloader, flash initialization or board success is implied.
 */
.option norelax
.section .text.start,"ax",@progbits
.global _start
_start:
    li sp, 0x2f07fff0
    li t0, 0x207041e8               /* PMU_PSRAM_XPD: actual MPLL PHY power gate */
    li t1, 0x80000000
    sw t1, 0(t0)
    li t0, 0x20500000
    li t1, 0x40000001               /* Enable/open Flash AXI interface, keep same-address check */
    sw t1, 0x3c(t0)
    li t1, 16                       /* Flash 64 KiB page */
    sw t1, 0x384(t0)
    sw zero, 0x380(t0)
    li t1, 0x1000                   /* valid, physical page 0 */
    sw t1, 0x37c(t0)
    li t0, 0x20502000
    li t1, 24                       /* PSRAM 8 KiB page */
    sw t1, 0x384(t0)
    sw zero, 0x380(t0)
    li t1, 0xc01                    /* valid, PSRAM, physical page 1 */
    sw t1, 0x37c(t0)
    li t0, 0x40000000
    jalr ra, t0, 0                  /* actual instruction fetch through MMU */
    li t1, 0x5a3c1729
    bne a0, t1, fail
    li t0, 0x50000000
    lw t2, 0(t0)
    bne t2, t1, fail
    li t0, 0x20502000
    li t1, 0xc00
    sw t1, 0x37c(t0)                /* same virtual address, different physical page */
    li t0, 0x50000000
    lw t1, 0(t0)
    bnez t1, fail
    li t0, 0x20502000
    li t1, 0xc01
    sw t1, 0x37c(t0)
    li t0, 0x50000000
    lw t1, 0(t0)
    bne t1, a0, fail
    la s0, pass
    jal ra, puts
1:  wfi
    j 1b
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
pass: .asciz "S31 MMU FLASH PSRAM PASS\n"
failed: .asciz "S31 MMU FAIL\n"

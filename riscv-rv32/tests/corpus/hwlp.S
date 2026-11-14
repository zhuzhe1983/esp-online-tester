# Every Xesploop (hardware loop) form, for the objdump differential decoder test (hwlp.dis): each
# operand field over all its values or bits, for both loops, then each form with one fixed bit
# (8-31) flipped and funct3 6/7. Assemble with riscv32-esp-elf-as
# -march=rv32imafc_zicsr_zifencei_xesploop. Flipped words objdump leaves as .insn must be illegal.
.attribute arch, "rv32imafc_zicsr_zifencei_xesploop"
.text
.option norvc
esp.lp.setup 0,zero,.+76
esp.lp.setup 0,ra,.+76
esp.lp.setup 0,sp,.+76
esp.lp.setup 0,gp,.+76
esp.lp.setup 0,tp,.+76
esp.lp.setup 0,t0,.+76
esp.lp.setup 0,t1,.+76
esp.lp.setup 0,t2,.+76
esp.lp.setup 0,s0,.+76
esp.lp.setup 0,s1,.+76
esp.lp.setup 0,a0,.+76
esp.lp.setup 0,a1,.+76
esp.lp.setup 0,a2,.+76
esp.lp.setup 0,a3,.+76
esp.lp.setup 0,a4,.+76
esp.lp.setup 0,a5,.+76
esp.lp.setup 0,a6,.+76
esp.lp.setup 0,a7,.+76
esp.lp.setup 0,s2,.+76
esp.lp.setup 0,s3,.+76
esp.lp.setup 0,s4,.+76
esp.lp.setup 0,s5,.+76
esp.lp.setup 0,s6,.+76
esp.lp.setup 0,s7,.+76
esp.lp.setup 0,s8,.+76
esp.lp.setup 0,s9,.+76
esp.lp.setup 0,s10,.+76
esp.lp.setup 0,s11,.+76
esp.lp.setup 0,t3,.+76
esp.lp.setup 0,t4,.+76
esp.lp.setup 0,t5,.+76
esp.lp.setup 0,t6,.+76
esp.lp.setup 0,a0,.+0
esp.lp.setup 0,a0,.+2
esp.lp.setup 0,a0,.+4
esp.lp.setup 0,a0,.+8
esp.lp.setup 0,a0,.+16
esp.lp.setup 0,a0,.+32
esp.lp.setup 0,a0,.+64
esp.lp.setup 0,a0,.+128
esp.lp.setup 0,a0,.+256
esp.lp.setup 0,a0,.+512
esp.lp.setup 0,a0,.+1024
esp.lp.setup 0,a0,.+2048
esp.lp.setup 0,a0,.+4096
esp.lp.setup 0,a0,.+8190
esp.lp.setup 0,a0,.+76
esp.lp.setupi 0,0,.+42
esp.lp.setupi 0,1,.+42
esp.lp.setupi 0,2,.+42
esp.lp.setupi 0,4,.+42
esp.lp.setupi 0,8,.+42
esp.lp.setupi 0,16,.+42
esp.lp.setupi 0,32,.+42
esp.lp.setupi 0,64,.+42
esp.lp.setupi 0,128,.+42
esp.lp.setupi 0,256,.+42
esp.lp.setupi 0,512,.+42
esp.lp.setupi 0,1024,.+42
esp.lp.setupi 0,2048,.+42
esp.lp.setupi 0,4095,.+42
esp.lp.setupi 0,5,.+0
esp.lp.setupi 0,5,.+2
esp.lp.setupi 0,5,.+4
esp.lp.setupi 0,5,.+8
esp.lp.setupi 0,5,.+16
esp.lp.setupi 0,5,.+32
esp.lp.setupi 0,5,.+64
esp.lp.setupi 0,5,.+128
esp.lp.setupi 0,5,.+256
esp.lp.setupi 0,5,.+512
esp.lp.setupi 0,5,.+1022
esp.lp.starti 0,.+0
esp.lp.starti 0,.+2
esp.lp.starti 0,.+4
esp.lp.starti 0,.+8
esp.lp.starti 0,.+16
esp.lp.starti 0,.+32
esp.lp.starti 0,.+64
esp.lp.starti 0,.+128
esp.lp.starti 0,.+256
esp.lp.starti 0,.+512
esp.lp.starti 0,.+1024
esp.lp.starti 0,.+2048
esp.lp.starti 0,.+4096
esp.lp.starti 0,.+8190
esp.lp.starti 0,.+76
esp.lp.endi 0,.+0
esp.lp.endi 0,.+2
esp.lp.endi 0,.+4
esp.lp.endi 0,.+8
esp.lp.endi 0,.+16
esp.lp.endi 0,.+32
esp.lp.endi 0,.+64
esp.lp.endi 0,.+128
esp.lp.endi 0,.+256
esp.lp.endi 0,.+512
esp.lp.endi 0,.+1024
esp.lp.endi 0,.+2048
esp.lp.endi 0,.+4096
esp.lp.endi 0,.+8190
esp.lp.endi 0,.+76
esp.lp.count 0,zero
esp.lp.count 0,ra
esp.lp.count 0,sp
esp.lp.count 0,gp
esp.lp.count 0,tp
esp.lp.count 0,t0
esp.lp.count 0,t1
esp.lp.count 0,t2
esp.lp.count 0,s0
esp.lp.count 0,s1
esp.lp.count 0,a0
esp.lp.count 0,a1
esp.lp.count 0,a2
esp.lp.count 0,a3
esp.lp.count 0,a4
esp.lp.count 0,a5
esp.lp.count 0,a6
esp.lp.count 0,a7
esp.lp.count 0,s2
esp.lp.count 0,s3
esp.lp.count 0,s4
esp.lp.count 0,s5
esp.lp.count 0,s6
esp.lp.count 0,s7
esp.lp.count 0,s8
esp.lp.count 0,s9
esp.lp.count 0,s10
esp.lp.count 0,s11
esp.lp.count 0,t3
esp.lp.count 0,t4
esp.lp.count 0,t5
esp.lp.count 0,t6
esp.lp.counti 0,0
esp.lp.counti 0,1
esp.lp.counti 0,2
esp.lp.counti 0,4
esp.lp.counti 0,8
esp.lp.counti 0,16
esp.lp.counti 0,32
esp.lp.counti 0,64
esp.lp.counti 0,128
esp.lp.counti 0,256
esp.lp.counti 0,512
esp.lp.counti 0,1024
esp.lp.counti 0,2048
esp.lp.counti 0,4095
esp.lp.setup 1,zero,.+76
esp.lp.setup 1,ra,.+76
esp.lp.setup 1,sp,.+76
esp.lp.setup 1,gp,.+76
esp.lp.setup 1,tp,.+76
esp.lp.setup 1,t0,.+76
esp.lp.setup 1,t1,.+76
esp.lp.setup 1,t2,.+76
esp.lp.setup 1,s0,.+76
esp.lp.setup 1,s1,.+76
esp.lp.setup 1,a0,.+76
esp.lp.setup 1,a1,.+76
esp.lp.setup 1,a2,.+76
esp.lp.setup 1,a3,.+76
esp.lp.setup 1,a4,.+76
esp.lp.setup 1,a5,.+76
esp.lp.setup 1,a6,.+76
esp.lp.setup 1,a7,.+76
esp.lp.setup 1,s2,.+76
esp.lp.setup 1,s3,.+76
esp.lp.setup 1,s4,.+76
esp.lp.setup 1,s5,.+76
esp.lp.setup 1,s6,.+76
esp.lp.setup 1,s7,.+76
esp.lp.setup 1,s8,.+76
esp.lp.setup 1,s9,.+76
esp.lp.setup 1,s10,.+76
esp.lp.setup 1,s11,.+76
esp.lp.setup 1,t3,.+76
esp.lp.setup 1,t4,.+76
esp.lp.setup 1,t5,.+76
esp.lp.setup 1,t6,.+76
esp.lp.setup 1,a0,.+0
esp.lp.setup 1,a0,.+2
esp.lp.setup 1,a0,.+4
esp.lp.setup 1,a0,.+8
esp.lp.setup 1,a0,.+16
esp.lp.setup 1,a0,.+32
esp.lp.setup 1,a0,.+64
esp.lp.setup 1,a0,.+128
esp.lp.setup 1,a0,.+256
esp.lp.setup 1,a0,.+512
esp.lp.setup 1,a0,.+1024
esp.lp.setup 1,a0,.+2048
esp.lp.setup 1,a0,.+4096
esp.lp.setup 1,a0,.+8190
esp.lp.setup 1,a0,.+76
esp.lp.setupi 1,0,.+42
esp.lp.setupi 1,1,.+42
esp.lp.setupi 1,2,.+42
esp.lp.setupi 1,4,.+42
esp.lp.setupi 1,8,.+42
esp.lp.setupi 1,16,.+42
esp.lp.setupi 1,32,.+42
esp.lp.setupi 1,64,.+42
esp.lp.setupi 1,128,.+42
esp.lp.setupi 1,256,.+42
esp.lp.setupi 1,512,.+42
esp.lp.setupi 1,1024,.+42
esp.lp.setupi 1,2048,.+42
esp.lp.setupi 1,4095,.+42
esp.lp.setupi 1,5,.+0
esp.lp.setupi 1,5,.+2
esp.lp.setupi 1,5,.+4
esp.lp.setupi 1,5,.+8
esp.lp.setupi 1,5,.+16
esp.lp.setupi 1,5,.+32
esp.lp.setupi 1,5,.+64
esp.lp.setupi 1,5,.+128
esp.lp.setupi 1,5,.+256
esp.lp.setupi 1,5,.+512
esp.lp.setupi 1,5,.+1022
esp.lp.starti 1,.+0
esp.lp.starti 1,.+2
esp.lp.starti 1,.+4
esp.lp.starti 1,.+8
esp.lp.starti 1,.+16
esp.lp.starti 1,.+32
esp.lp.starti 1,.+64
esp.lp.starti 1,.+128
esp.lp.starti 1,.+256
esp.lp.starti 1,.+512
esp.lp.starti 1,.+1024
esp.lp.starti 1,.+2048
esp.lp.starti 1,.+4096
esp.lp.starti 1,.+8190
esp.lp.starti 1,.+76
esp.lp.endi 1,.+0
esp.lp.endi 1,.+2
esp.lp.endi 1,.+4
esp.lp.endi 1,.+8
esp.lp.endi 1,.+16
esp.lp.endi 1,.+32
esp.lp.endi 1,.+64
esp.lp.endi 1,.+128
esp.lp.endi 1,.+256
esp.lp.endi 1,.+512
esp.lp.endi 1,.+1024
esp.lp.endi 1,.+2048
esp.lp.endi 1,.+4096
esp.lp.endi 1,.+8190
esp.lp.endi 1,.+76
esp.lp.count 1,zero
esp.lp.count 1,ra
esp.lp.count 1,sp
esp.lp.count 1,gp
esp.lp.count 1,tp
esp.lp.count 1,t0
esp.lp.count 1,t1
esp.lp.count 1,t2
esp.lp.count 1,s0
esp.lp.count 1,s1
esp.lp.count 1,a0
esp.lp.count 1,a1
esp.lp.count 1,a2
esp.lp.count 1,a3
esp.lp.count 1,a4
esp.lp.count 1,a5
esp.lp.count 1,a6
esp.lp.count 1,a7
esp.lp.count 1,s2
esp.lp.count 1,s3
esp.lp.count 1,s4
esp.lp.count 1,s5
esp.lp.count 1,s6
esp.lp.count 1,s7
esp.lp.count 1,s8
esp.lp.count 1,s9
esp.lp.count 1,s10
esp.lp.count 1,s11
esp.lp.count 1,t3
esp.lp.count 1,t4
esp.lp.count 1,t5
esp.lp.count 1,t6
esp.lp.counti 1,0
esp.lp.counti 1,1
esp.lp.counti 1,2
esp.lp.counti 1,4
esp.lp.counti 1,8
esp.lp.counti 1,16
esp.lp.counti 1,32
esp.lp.counti 1,64
esp.lp.counti 1,128
esp.lp.counti 1,256
esp.lp.counti 1,512
esp.lp.counti 1,1024
esp.lp.counti 1,2048
esp.lp.counti 1,4095
.insn 4, 0x0000012b
.insn 4, 0x0000022b
.insn 4, 0x0000042b
.insn 4, 0x0000082b
.insn 4, 0x0000802b
.insn 4, 0x0001002b
.insn 4, 0x0002002b
.insn 4, 0x0004002b
.insn 4, 0x0008002b
.insn 4, 0x0000112b
.insn 4, 0x0000122b
.insn 4, 0x0000142b
.insn 4, 0x0000182b
.insn 4, 0x0000902b
.insn 4, 0x0001102b
.insn 4, 0x0002102b
.insn 4, 0x0004102b
.insn 4, 0x0008102b
.insn 4, 0x0000212b
.insn 4, 0x0000222b
.insn 4, 0x0000242b
.insn 4, 0x0000282b
.insn 4, 0x0010202b
.insn 4, 0x0020202b
.insn 4, 0x0040202b
.insn 4, 0x0080202b
.insn 4, 0x0100202b
.insn 4, 0x0200202b
.insn 4, 0x0400202b
.insn 4, 0x0800202b
.insn 4, 0x1000202b
.insn 4, 0x2000202b
.insn 4, 0x4000202b
.insn 4, 0x8000202b
.insn 4, 0x0000312b
.insn 4, 0x0000322b
.insn 4, 0x0000342b
.insn 4, 0x0000382b
.insn 4, 0x0000b02b
.insn 4, 0x0001302b
.insn 4, 0x0002302b
.insn 4, 0x0004302b
.insn 4, 0x0008302b
.insn 4, 0x0000412b
.insn 4, 0x0000422b
.insn 4, 0x0000442b
.insn 4, 0x0000482b
.insn 4, 0x0000602b
.insn 4, 0x000060ab
.insn 4, 0x0000702b
.insn 4, 0x000070ab

# Every form of the Xespv2p2 (PIE v2.2) subset the ESP32-S31 core models, for the objdump
# differential decoder test (pie.dis): each operand field over all its values, every sat/rm pair,
# then each form with one fixed bit (7-31) flipped. Assemble with riscv32-esp-elf-as-xespv2p2
# -march=rv32imafc_zicsr_zifencei_xespv2p2. Flipped words objdump leaves as .insn must be illegal;
# those it names as other esp.* instructions are outside the subset and must not decode as one of it.
.text
.option norvc
esp.vmul.u16 q0,q4,q7,sat,dyn
esp.vmul.u16 q1,q4,q7,sat,dyn
esp.vmul.u16 q2,q4,q7,sat,dyn
esp.vmul.u16 q3,q4,q7,sat,dyn
esp.vmul.u16 q4,q4,q7,sat,dyn
esp.vmul.u16 q5,q4,q7,sat,dyn
esp.vmul.u16 q6,q4,q7,sat,dyn
esp.vmul.u16 q7,q4,q7,sat,dyn
esp.vmul.u16 q1,q0,q7,sat,dyn
esp.vmul.u16 q1,q1,q7,sat,dyn
esp.vmul.u16 q1,q2,q7,sat,dyn
esp.vmul.u16 q1,q3,q7,sat,dyn
esp.vmul.u16 q1,q5,q7,sat,dyn
esp.vmul.u16 q1,q6,q7,sat,dyn
esp.vmul.u16 q1,q7,q7,sat,dyn
esp.vmul.u16 q1,q4,q0,sat,dyn
esp.vmul.u16 q1,q4,q1,sat,dyn
esp.vmul.u16 q1,q4,q2,sat,dyn
esp.vmul.u16 q1,q4,q3,sat,dyn
esp.vmul.u16 q1,q4,q4,sat,dyn
esp.vmul.u16 q1,q4,q5,sat,dyn
esp.vmul.u16 q1,q4,q6,sat,dyn
esp.vmul.u16 q1,q4,q7,trunc,dyn
esp.vmul.u16 q1,q4,q7,sat,rdn
esp.vmul.u16 q1,q4,q7,sat,rup
esp.vmul.u16 q1,q4,q7,sat,raz
esp.vmul.u16 q1,q4,q7,sat,rtz
esp.vmul.u16 q1,q4,q7,sat,rhaz
esp.vmul.u16 q1,q4,q7,sat,rhtz
esp.vmul.u16 q1,q4,q7,sat,rne
esp.vmul.u16 q1,q4,q7,trunc,rdn
esp.vmul.u16 q1,q4,q7,trunc,rup
esp.vmul.u16 q1,q4,q7,trunc,raz
esp.vmul.u16 q1,q4,q7,trunc,rtz
esp.vmul.u16 q1,q4,q7,trunc,rhaz
esp.vmul.u16 q1,q4,q7,trunc,rhtz
esp.vmul.u16 q1,q4,q7,trunc,rne
esp.vmul.u16.ld.incp q0,a0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q2,a0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q3,a0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q4,a0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q5,a0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q6,a0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q7,a0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,s0,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,s1,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a1,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a2,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a3,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a4,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a5,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,s8,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,s9,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,s10,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,s11,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,t3,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,t4,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,t5,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,t6,q7,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q0,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q1,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q2,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q3,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q4,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q5,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q6,q2,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q0,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q1,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q3,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q4,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q5,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q6,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q7,q5,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q0,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q1,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q2,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q3,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q4,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q6,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q7,sat,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,trunc,dyn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,sat,rdn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,sat,rup
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,sat,raz
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,sat,rtz
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,sat,rhaz
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,sat,rhtz
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,sat,rne
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,trunc,rdn
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,trunc,rup
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,trunc,raz
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,trunc,rtz
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,trunc,rhaz
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,trunc,rhtz
esp.vmul.u16.ld.incp q1,a0,q7,q2,q5,trunc,rne
esp.vadd.u16 q0,q4,q7,sat
esp.vadd.u16 q1,q4,q7,sat
esp.vadd.u16 q2,q4,q7,sat
esp.vadd.u16 q3,q4,q7,sat
esp.vadd.u16 q4,q4,q7,sat
esp.vadd.u16 q5,q4,q7,sat
esp.vadd.u16 q6,q4,q7,sat
esp.vadd.u16 q7,q4,q7,sat
esp.vadd.u16 q1,q0,q7,sat
esp.vadd.u16 q1,q1,q7,sat
esp.vadd.u16 q1,q2,q7,sat
esp.vadd.u16 q1,q3,q7,sat
esp.vadd.u16 q1,q5,q7,sat
esp.vadd.u16 q1,q6,q7,sat
esp.vadd.u16 q1,q7,q7,sat
esp.vadd.u16 q1,q4,q0,sat
esp.vadd.u16 q1,q4,q1,sat
esp.vadd.u16 q1,q4,q2,sat
esp.vadd.u16 q1,q4,q3,sat
esp.vadd.u16 q1,q4,q4,sat
esp.vadd.u16 q1,q4,q5,sat
esp.vadd.u16 q1,q4,q6,sat
esp.vadd.u16 q1,q4,q7,trunc
esp.vadd.u16.ld.incp q0,a0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q2,a0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q3,a0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q4,a0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q5,a0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q6,a0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q7,a0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,s0,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,s1,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,a1,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,a2,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,a3,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,a4,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,a5,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,s8,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,s9,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,s10,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,s11,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,t3,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,t4,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,t5,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,t6,q7,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q0,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q1,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q2,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q3,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q4,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q5,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q6,q2,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q0,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q1,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q3,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q4,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q5,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q6,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q7,q5,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q0,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q1,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q2,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q3,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q4,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q6,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q7,sat
esp.vadd.u16.ld.incp q1,a0,q7,q2,q5,trunc
esp.vsub.u16 q0,q4,q7,sat
esp.vsub.u16 q1,q4,q7,sat
esp.vsub.u16 q2,q4,q7,sat
esp.vsub.u16 q3,q4,q7,sat
esp.vsub.u16 q4,q4,q7,sat
esp.vsub.u16 q5,q4,q7,sat
esp.vsub.u16 q6,q4,q7,sat
esp.vsub.u16 q7,q4,q7,sat
esp.vsub.u16 q1,q0,q7,sat
esp.vsub.u16 q1,q1,q7,sat
esp.vsub.u16 q1,q2,q7,sat
esp.vsub.u16 q1,q3,q7,sat
esp.vsub.u16 q1,q5,q7,sat
esp.vsub.u16 q1,q6,q7,sat
esp.vsub.u16 q1,q7,q7,sat
esp.vsub.u16 q1,q4,q0,sat
esp.vsub.u16 q1,q4,q1,sat
esp.vsub.u16 q1,q4,q2,sat
esp.vsub.u16 q1,q4,q3,sat
esp.vsub.u16 q1,q4,q4,sat
esp.vsub.u16 q1,q4,q5,sat
esp.vsub.u16 q1,q4,q6,sat
esp.vsub.u16 q1,q4,q7,trunc
esp.vsub.u8 q0,q4,q7,sat
esp.vsub.u8 q1,q4,q7,sat
esp.vsub.u8 q2,q4,q7,sat
esp.vsub.u8 q3,q4,q7,sat
esp.vsub.u8 q4,q4,q7,sat
esp.vsub.u8 q5,q4,q7,sat
esp.vsub.u8 q6,q4,q7,sat
esp.vsub.u8 q7,q4,q7,sat
esp.vsub.u8 q1,q0,q7,sat
esp.vsub.u8 q1,q1,q7,sat
esp.vsub.u8 q1,q2,q7,sat
esp.vsub.u8 q1,q3,q7,sat
esp.vsub.u8 q1,q5,q7,sat
esp.vsub.u8 q1,q6,q7,sat
esp.vsub.u8 q1,q7,q7,sat
esp.vsub.u8 q1,q4,q0,sat
esp.vsub.u8 q1,q4,q1,sat
esp.vsub.u8 q1,q4,q2,sat
esp.vsub.u8 q1,q4,q3,sat
esp.vsub.u8 q1,q4,q4,sat
esp.vsub.u8 q1,q4,q5,sat
esp.vsub.u8 q1,q4,q6,sat
esp.vsub.u8 q1,q4,q7,trunc
esp.vmin.u16 q0,q4,q7
esp.vmin.u16 q1,q4,q7
esp.vmin.u16 q2,q4,q7
esp.vmin.u16 q3,q4,q7
esp.vmin.u16 q4,q4,q7
esp.vmin.u16 q5,q4,q7
esp.vmin.u16 q6,q4,q7
esp.vmin.u16 q7,q4,q7
esp.vmin.u16 q1,q0,q7
esp.vmin.u16 q1,q1,q7
esp.vmin.u16 q1,q2,q7
esp.vmin.u16 q1,q3,q7
esp.vmin.u16 q1,q5,q7
esp.vmin.u16 q1,q6,q7
esp.vmin.u16 q1,q7,q7
esp.vmin.u16 q1,q4,q0
esp.vmin.u16 q1,q4,q1
esp.vmin.u16 q1,q4,q2
esp.vmin.u16 q1,q4,q3
esp.vmin.u16 q1,q4,q4
esp.vmin.u16 q1,q4,q5
esp.vmin.u16 q1,q4,q6
esp.vcmp.eq.u16 q0,q4,q7
esp.vcmp.eq.u16 q1,q4,q7
esp.vcmp.eq.u16 q2,q4,q7
esp.vcmp.eq.u16 q3,q4,q7
esp.vcmp.eq.u16 q4,q4,q7
esp.vcmp.eq.u16 q5,q4,q7
esp.vcmp.eq.u16 q6,q4,q7
esp.vcmp.eq.u16 q7,q4,q7
esp.vcmp.eq.u16 q1,q0,q7
esp.vcmp.eq.u16 q1,q1,q7
esp.vcmp.eq.u16 q1,q2,q7
esp.vcmp.eq.u16 q1,q3,q7
esp.vcmp.eq.u16 q1,q5,q7
esp.vcmp.eq.u16 q1,q6,q7
esp.vcmp.eq.u16 q1,q7,q7
esp.vcmp.eq.u16 q1,q4,q0
esp.vcmp.eq.u16 q1,q4,q1
esp.vcmp.eq.u16 q1,q4,q2
esp.vcmp.eq.u16 q1,q4,q3
esp.vcmp.eq.u16 q1,q4,q4
esp.vcmp.eq.u16 q1,q4,q5
esp.vcmp.eq.u16 q1,q4,q6
esp.andq q0,q4,q7
esp.andq q1,q4,q7
esp.andq q2,q4,q7
esp.andq q3,q4,q7
esp.andq q4,q4,q7
esp.andq q5,q4,q7
esp.andq q6,q4,q7
esp.andq q7,q4,q7
esp.andq q1,q0,q7
esp.andq q1,q1,q7
esp.andq q1,q2,q7
esp.andq q1,q3,q7
esp.andq q1,q5,q7
esp.andq q1,q6,q7
esp.andq q1,q7,q7
esp.andq q1,q4,q0
esp.andq q1,q4,q1
esp.andq q1,q4,q2
esp.andq q1,q4,q3
esp.andq q1,q4,q4
esp.andq q1,q4,q5
esp.andq q1,q4,q6
esp.orq q0,q4,q7
esp.orq q1,q4,q7
esp.orq q2,q4,q7
esp.orq q3,q4,q7
esp.orq q4,q4,q7
esp.orq q5,q4,q7
esp.orq q6,q4,q7
esp.orq q7,q4,q7
esp.orq q1,q0,q7
esp.orq q1,q1,q7
esp.orq q1,q2,q7
esp.orq q1,q3,q7
esp.orq q1,q5,q7
esp.orq q1,q6,q7
esp.orq q1,q7,q7
esp.orq q1,q4,q0
esp.orq q1,q4,q1
esp.orq q1,q4,q2
esp.orq q1,q4,q3
esp.orq q1,q4,q4
esp.orq q1,q4,q5
esp.orq q1,q4,q6
esp.xorq q0,q4,q7
esp.xorq q1,q4,q7
esp.xorq q2,q4,q7
esp.xorq q3,q4,q7
esp.xorq q4,q4,q7
esp.xorq q5,q4,q7
esp.xorq q6,q4,q7
esp.xorq q7,q4,q7
esp.xorq q1,q0,q7
esp.xorq q1,q1,q7
esp.xorq q1,q2,q7
esp.xorq q1,q3,q7
esp.xorq q1,q5,q7
esp.xorq q1,q6,q7
esp.xorq q1,q7,q7
esp.xorq q1,q4,q0
esp.xorq q1,q4,q1
esp.xorq q1,q4,q2
esp.xorq q1,q4,q3
esp.xorq q1,q4,q4
esp.xorq q1,q4,q5
esp.xorq q1,q4,q6
esp.zero.q q0
esp.zero.q q1
esp.zero.q q2
esp.zero.q q3
esp.zero.q q4
esp.zero.q q5
esp.zero.q q6
esp.zero.q q7
esp.vext.u8 q0,q4,q7
esp.vext.u8 q1,q4,q7
esp.vext.u8 q2,q4,q7
esp.vext.u8 q3,q4,q7
esp.vext.u8 q4,q4,q7
esp.vext.u8 q5,q4,q7
esp.vext.u8 q6,q4,q7
esp.vext.u8 q7,q4,q7
esp.vext.u8 q1,q0,q7
esp.vext.u8 q1,q1,q7
esp.vext.u8 q1,q2,q7
esp.vext.u8 q1,q3,q7
esp.vext.u8 q1,q5,q7
esp.vext.u8 q1,q6,q7
esp.vext.u8 q1,q7,q7
esp.vext.u8 q1,q4,q0
esp.vext.u8 q1,q4,q1
esp.vext.u8 q1,q4,q2
esp.vext.u8 q1,q4,q3
esp.vext.u8 q1,q4,q4
esp.vext.u8 q1,q4,q5
esp.vext.u8 q1,q4,q6
esp.vunzip.8 q0,q4
esp.vunzip.8 q1,q4
esp.vunzip.8 q2,q4
esp.vunzip.8 q3,q4
esp.vunzip.8 q4,q4
esp.vunzip.8 q5,q4
esp.vunzip.8 q6,q4
esp.vunzip.8 q7,q4
esp.vunzip.8 q1,q0
esp.vunzip.8 q1,q1
esp.vunzip.8 q1,q2
esp.vunzip.8 q1,q3
esp.vunzip.8 q1,q5
esp.vunzip.8 q1,q6
esp.vunzip.8 q1,q7
esp.vzipt.8 q0,q4,q7
esp.vzipt.8 q1,q4,q7
esp.vzipt.8 q2,q4,q7
esp.vzipt.8 q3,q4,q7
esp.vzipt.8 q4,q4,q7
esp.vzipt.8 q5,q4,q7
esp.vzipt.8 q6,q4,q7
esp.vzipt.8 q7,q4,q7
esp.vzipt.8 q1,q0,q7
esp.vzipt.8 q1,q1,q7
esp.vzipt.8 q1,q2,q7
esp.vzipt.8 q1,q3,q7
esp.vzipt.8 q1,q5,q7
esp.vzipt.8 q1,q6,q7
esp.vzipt.8 q1,q7,q7
esp.vzipt.8 q1,q4,q0
esp.vzipt.8 q1,q4,q1
esp.vzipt.8 q1,q4,q2
esp.vzipt.8 q1,q4,q3
esp.vzipt.8 q1,q4,q4
esp.vzipt.8 q1,q4,q5
esp.vzipt.8 q1,q4,q6
esp.vunzipt.8 q0,q4,q7
esp.vunzipt.8 q1,q4,q7
esp.vunzipt.8 q2,q4,q7
esp.vunzipt.8 q3,q4,q7
esp.vunzipt.8 q4,q4,q7
esp.vunzipt.8 q5,q4,q7
esp.vunzipt.8 q6,q4,q7
esp.vunzipt.8 q7,q4,q7
esp.vunzipt.8 q1,q0,q7
esp.vunzipt.8 q1,q1,q7
esp.vunzipt.8 q1,q2,q7
esp.vunzipt.8 q1,q3,q7
esp.vunzipt.8 q1,q5,q7
esp.vunzipt.8 q1,q6,q7
esp.vunzipt.8 q1,q7,q7
esp.vunzipt.8 q1,q4,q0
esp.vunzipt.8 q1,q4,q1
esp.vunzipt.8 q1,q4,q2
esp.vunzipt.8 q1,q4,q3
esp.vunzipt.8 q1,q4,q4
esp.vunzipt.8 q1,q4,q5
esp.vunzipt.8 q1,q4,q6
esp.src.q q0,q4,q7
esp.src.q q1,q4,q7
esp.src.q q2,q4,q7
esp.src.q q3,q4,q7
esp.src.q q4,q4,q7
esp.src.q q5,q4,q7
esp.src.q q6,q4,q7
esp.src.q q7,q4,q7
esp.src.q q1,q0,q7
esp.src.q q1,q1,q7
esp.src.q q1,q2,q7
esp.src.q q1,q3,q7
esp.src.q q1,q5,q7
esp.src.q q1,q6,q7
esp.src.q q1,q7,q7
esp.src.q q1,q4,q0
esp.src.q q1,q4,q1
esp.src.q q1,q4,q2
esp.src.q q1,q4,q3
esp.src.q q1,q4,q4
esp.src.q q1,q4,q5
esp.src.q q1,q4,q6
esp.vld.128.ip q0,a0,16
esp.vld.128.ip q1,a0,16
esp.vld.128.ip q2,a0,16
esp.vld.128.ip q3,a0,16
esp.vld.128.ip q4,a0,16
esp.vld.128.ip q5,a0,16
esp.vld.128.ip q6,a0,16
esp.vld.128.ip q7,a0,16
esp.vld.128.ip q1,s0,16
esp.vld.128.ip q1,s1,16
esp.vld.128.ip q1,a1,16
esp.vld.128.ip q1,a2,16
esp.vld.128.ip q1,a3,16
esp.vld.128.ip q1,a4,16
esp.vld.128.ip q1,a5,16
esp.vld.128.ip q1,s8,16
esp.vld.128.ip q1,s9,16
esp.vld.128.ip q1,s10,16
esp.vld.128.ip q1,s11,16
esp.vld.128.ip q1,t3,16
esp.vld.128.ip q1,t4,16
esp.vld.128.ip q1,t5,16
esp.vld.128.ip q1,t6,16
esp.vld.128.ip q1,a0,-2048
esp.vld.128.ip q1,a0,-2032
esp.vld.128.ip q1,a0,-2016
esp.vld.128.ip q1,a0,-2000
esp.vld.128.ip q1,a0,-1984
esp.vld.128.ip q1,a0,-1968
esp.vld.128.ip q1,a0,-1952
esp.vld.128.ip q1,a0,-1936
esp.vld.128.ip q1,a0,-1920
esp.vld.128.ip q1,a0,-1904
esp.vld.128.ip q1,a0,-1888
esp.vld.128.ip q1,a0,-1872
esp.vld.128.ip q1,a0,-1856
esp.vld.128.ip q1,a0,-1840
esp.vld.128.ip q1,a0,-1824
esp.vld.128.ip q1,a0,-1808
esp.vld.128.ip q1,a0,-1792
esp.vld.128.ip q1,a0,-1776
esp.vld.128.ip q1,a0,-1760
esp.vld.128.ip q1,a0,-1744
esp.vld.128.ip q1,a0,-1728
esp.vld.128.ip q1,a0,-1712
esp.vld.128.ip q1,a0,-1696
esp.vld.128.ip q1,a0,-1680
esp.vld.128.ip q1,a0,-1664
esp.vld.128.ip q1,a0,-1648
esp.vld.128.ip q1,a0,-1632
esp.vld.128.ip q1,a0,-1616
esp.vld.128.ip q1,a0,-1600
esp.vld.128.ip q1,a0,-1584
esp.vld.128.ip q1,a0,-1568
esp.vld.128.ip q1,a0,-1552
esp.vld.128.ip q1,a0,-1536
esp.vld.128.ip q1,a0,-1520
esp.vld.128.ip q1,a0,-1504
esp.vld.128.ip q1,a0,-1488
esp.vld.128.ip q1,a0,-1472
esp.vld.128.ip q1,a0,-1456
esp.vld.128.ip q1,a0,-1440
esp.vld.128.ip q1,a0,-1424
esp.vld.128.ip q1,a0,-1408
esp.vld.128.ip q1,a0,-1392
esp.vld.128.ip q1,a0,-1376
esp.vld.128.ip q1,a0,-1360
esp.vld.128.ip q1,a0,-1344
esp.vld.128.ip q1,a0,-1328
esp.vld.128.ip q1,a0,-1312
esp.vld.128.ip q1,a0,-1296
esp.vld.128.ip q1,a0,-1280
esp.vld.128.ip q1,a0,-1264
esp.vld.128.ip q1,a0,-1248
esp.vld.128.ip q1,a0,-1232
esp.vld.128.ip q1,a0,-1216
esp.vld.128.ip q1,a0,-1200
esp.vld.128.ip q1,a0,-1184
esp.vld.128.ip q1,a0,-1168
esp.vld.128.ip q1,a0,-1152
esp.vld.128.ip q1,a0,-1136
esp.vld.128.ip q1,a0,-1120
esp.vld.128.ip q1,a0,-1104
esp.vld.128.ip q1,a0,-1088
esp.vld.128.ip q1,a0,-1072
esp.vld.128.ip q1,a0,-1056
esp.vld.128.ip q1,a0,-1040
esp.vld.128.ip q1,a0,-1024
esp.vld.128.ip q1,a0,-1008
esp.vld.128.ip q1,a0,-992
esp.vld.128.ip q1,a0,-976
esp.vld.128.ip q1,a0,-960
esp.vld.128.ip q1,a0,-944
esp.vld.128.ip q1,a0,-928
esp.vld.128.ip q1,a0,-912
esp.vld.128.ip q1,a0,-896
esp.vld.128.ip q1,a0,-880
esp.vld.128.ip q1,a0,-864
esp.vld.128.ip q1,a0,-848
esp.vld.128.ip q1,a0,-832
esp.vld.128.ip q1,a0,-816
esp.vld.128.ip q1,a0,-800
esp.vld.128.ip q1,a0,-784
esp.vld.128.ip q1,a0,-768
esp.vld.128.ip q1,a0,-752
esp.vld.128.ip q1,a0,-736
esp.vld.128.ip q1,a0,-720
esp.vld.128.ip q1,a0,-704
esp.vld.128.ip q1,a0,-688
esp.vld.128.ip q1,a0,-672
esp.vld.128.ip q1,a0,-656
esp.vld.128.ip q1,a0,-640
esp.vld.128.ip q1,a0,-624
esp.vld.128.ip q1,a0,-608
esp.vld.128.ip q1,a0,-592
esp.vld.128.ip q1,a0,-576
esp.vld.128.ip q1,a0,-560
esp.vld.128.ip q1,a0,-544
esp.vld.128.ip q1,a0,-528
esp.vld.128.ip q1,a0,-512
esp.vld.128.ip q1,a0,-496
esp.vld.128.ip q1,a0,-480
esp.vld.128.ip q1,a0,-464
esp.vld.128.ip q1,a0,-448
esp.vld.128.ip q1,a0,-432
esp.vld.128.ip q1,a0,-416
esp.vld.128.ip q1,a0,-400
esp.vld.128.ip q1,a0,-384
esp.vld.128.ip q1,a0,-368
esp.vld.128.ip q1,a0,-352
esp.vld.128.ip q1,a0,-336
esp.vld.128.ip q1,a0,-320
esp.vld.128.ip q1,a0,-304
esp.vld.128.ip q1,a0,-288
esp.vld.128.ip q1,a0,-272
esp.vld.128.ip q1,a0,-256
esp.vld.128.ip q1,a0,-240
esp.vld.128.ip q1,a0,-224
esp.vld.128.ip q1,a0,-208
esp.vld.128.ip q1,a0,-192
esp.vld.128.ip q1,a0,-176
esp.vld.128.ip q1,a0,-160
esp.vld.128.ip q1,a0,-144
esp.vld.128.ip q1,a0,-128
esp.vld.128.ip q1,a0,-112
esp.vld.128.ip q1,a0,-96
esp.vld.128.ip q1,a0,-80
esp.vld.128.ip q1,a0,-64
esp.vld.128.ip q1,a0,-48
esp.vld.128.ip q1,a0,-32
esp.vld.128.ip q1,a0,-16
esp.vld.128.ip q1,a0,0
esp.vld.128.ip q1,a0,32
esp.vld.128.ip q1,a0,48
esp.vld.128.ip q1,a0,64
esp.vld.128.ip q1,a0,80
esp.vld.128.ip q1,a0,96
esp.vld.128.ip q1,a0,112
esp.vld.128.ip q1,a0,128
esp.vld.128.ip q1,a0,144
esp.vld.128.ip q1,a0,160
esp.vld.128.ip q1,a0,176
esp.vld.128.ip q1,a0,192
esp.vld.128.ip q1,a0,208
esp.vld.128.ip q1,a0,224
esp.vld.128.ip q1,a0,240
esp.vld.128.ip q1,a0,256
esp.vld.128.ip q1,a0,272
esp.vld.128.ip q1,a0,288
esp.vld.128.ip q1,a0,304
esp.vld.128.ip q1,a0,320
esp.vld.128.ip q1,a0,336
esp.vld.128.ip q1,a0,352
esp.vld.128.ip q1,a0,368
esp.vld.128.ip q1,a0,384
esp.vld.128.ip q1,a0,400
esp.vld.128.ip q1,a0,416
esp.vld.128.ip q1,a0,432
esp.vld.128.ip q1,a0,448
esp.vld.128.ip q1,a0,464
esp.vld.128.ip q1,a0,480
esp.vld.128.ip q1,a0,496
esp.vld.128.ip q1,a0,512
esp.vld.128.ip q1,a0,528
esp.vld.128.ip q1,a0,544
esp.vld.128.ip q1,a0,560
esp.vld.128.ip q1,a0,576
esp.vld.128.ip q1,a0,592
esp.vld.128.ip q1,a0,608
esp.vld.128.ip q1,a0,624
esp.vld.128.ip q1,a0,640
esp.vld.128.ip q1,a0,656
esp.vld.128.ip q1,a0,672
esp.vld.128.ip q1,a0,688
esp.vld.128.ip q1,a0,704
esp.vld.128.ip q1,a0,720
esp.vld.128.ip q1,a0,736
esp.vld.128.ip q1,a0,752
esp.vld.128.ip q1,a0,768
esp.vld.128.ip q1,a0,784
esp.vld.128.ip q1,a0,800
esp.vld.128.ip q1,a0,816
esp.vld.128.ip q1,a0,832
esp.vld.128.ip q1,a0,848
esp.vld.128.ip q1,a0,864
esp.vld.128.ip q1,a0,880
esp.vld.128.ip q1,a0,896
esp.vld.128.ip q1,a0,912
esp.vld.128.ip q1,a0,928
esp.vld.128.ip q1,a0,944
esp.vld.128.ip q1,a0,960
esp.vld.128.ip q1,a0,976
esp.vld.128.ip q1,a0,992
esp.vld.128.ip q1,a0,1008
esp.vld.128.ip q1,a0,1024
esp.vld.128.ip q1,a0,1040
esp.vld.128.ip q1,a0,1056
esp.vld.128.ip q1,a0,1072
esp.vld.128.ip q1,a0,1088
esp.vld.128.ip q1,a0,1104
esp.vld.128.ip q1,a0,1120
esp.vld.128.ip q1,a0,1136
esp.vld.128.ip q1,a0,1152
esp.vld.128.ip q1,a0,1168
esp.vld.128.ip q1,a0,1184
esp.vld.128.ip q1,a0,1200
esp.vld.128.ip q1,a0,1216
esp.vld.128.ip q1,a0,1232
esp.vld.128.ip q1,a0,1248
esp.vld.128.ip q1,a0,1264
esp.vld.128.ip q1,a0,1280
esp.vld.128.ip q1,a0,1296
esp.vld.128.ip q1,a0,1312
esp.vld.128.ip q1,a0,1328
esp.vld.128.ip q1,a0,1344
esp.vld.128.ip q1,a0,1360
esp.vld.128.ip q1,a0,1376
esp.vld.128.ip q1,a0,1392
esp.vld.128.ip q1,a0,1408
esp.vld.128.ip q1,a0,1424
esp.vld.128.ip q1,a0,1440
esp.vld.128.ip q1,a0,1456
esp.vld.128.ip q1,a0,1472
esp.vld.128.ip q1,a0,1488
esp.vld.128.ip q1,a0,1504
esp.vld.128.ip q1,a0,1520
esp.vld.128.ip q1,a0,1536
esp.vld.128.ip q1,a0,1552
esp.vld.128.ip q1,a0,1568
esp.vld.128.ip q1,a0,1584
esp.vld.128.ip q1,a0,1600
esp.vld.128.ip q1,a0,1616
esp.vld.128.ip q1,a0,1632
esp.vld.128.ip q1,a0,1648
esp.vld.128.ip q1,a0,1664
esp.vld.128.ip q1,a0,1680
esp.vld.128.ip q1,a0,1696
esp.vld.128.ip q1,a0,1712
esp.vld.128.ip q1,a0,1728
esp.vld.128.ip q1,a0,1744
esp.vld.128.ip q1,a0,1760
esp.vld.128.ip q1,a0,1776
esp.vld.128.ip q1,a0,1792
esp.vld.128.ip q1,a0,1808
esp.vld.128.ip q1,a0,1824
esp.vld.128.ip q1,a0,1840
esp.vld.128.ip q1,a0,1856
esp.vld.128.ip q1,a0,1872
esp.vld.128.ip q1,a0,1888
esp.vld.128.ip q1,a0,1904
esp.vld.128.ip q1,a0,1920
esp.vld.128.ip q1,a0,1936
esp.vld.128.ip q1,a0,1952
esp.vld.128.ip q1,a0,1968
esp.vld.128.ip q1,a0,1984
esp.vld.128.ip q1,a0,2000
esp.vld.128.ip q1,a0,2016
esp.vld.128.ip q1,a0,2032
esp.vst.128.ip q0,a0,16
esp.vst.128.ip q1,a0,16
esp.vst.128.ip q2,a0,16
esp.vst.128.ip q3,a0,16
esp.vst.128.ip q4,a0,16
esp.vst.128.ip q5,a0,16
esp.vst.128.ip q6,a0,16
esp.vst.128.ip q7,a0,16
esp.vst.128.ip q1,s0,16
esp.vst.128.ip q1,s1,16
esp.vst.128.ip q1,a1,16
esp.vst.128.ip q1,a2,16
esp.vst.128.ip q1,a3,16
esp.vst.128.ip q1,a4,16
esp.vst.128.ip q1,a5,16
esp.vst.128.ip q1,s8,16
esp.vst.128.ip q1,s9,16
esp.vst.128.ip q1,s10,16
esp.vst.128.ip q1,s11,16
esp.vst.128.ip q1,t3,16
esp.vst.128.ip q1,t4,16
esp.vst.128.ip q1,t5,16
esp.vst.128.ip q1,t6,16
esp.vst.128.ip q1,a0,-2048
esp.vst.128.ip q1,a0,-2032
esp.vst.128.ip q1,a0,-2016
esp.vst.128.ip q1,a0,-2000
esp.vst.128.ip q1,a0,-1984
esp.vst.128.ip q1,a0,-1968
esp.vst.128.ip q1,a0,-1952
esp.vst.128.ip q1,a0,-1936
esp.vst.128.ip q1,a0,-1920
esp.vst.128.ip q1,a0,-1904
esp.vst.128.ip q1,a0,-1888
esp.vst.128.ip q1,a0,-1872
esp.vst.128.ip q1,a0,-1856
esp.vst.128.ip q1,a0,-1840
esp.vst.128.ip q1,a0,-1824
esp.vst.128.ip q1,a0,-1808
esp.vst.128.ip q1,a0,-1792
esp.vst.128.ip q1,a0,-1776
esp.vst.128.ip q1,a0,-1760
esp.vst.128.ip q1,a0,-1744
esp.vst.128.ip q1,a0,-1728
esp.vst.128.ip q1,a0,-1712
esp.vst.128.ip q1,a0,-1696
esp.vst.128.ip q1,a0,-1680
esp.vst.128.ip q1,a0,-1664
esp.vst.128.ip q1,a0,-1648
esp.vst.128.ip q1,a0,-1632
esp.vst.128.ip q1,a0,-1616
esp.vst.128.ip q1,a0,-1600
esp.vst.128.ip q1,a0,-1584
esp.vst.128.ip q1,a0,-1568
esp.vst.128.ip q1,a0,-1552
esp.vst.128.ip q1,a0,-1536
esp.vst.128.ip q1,a0,-1520
esp.vst.128.ip q1,a0,-1504
esp.vst.128.ip q1,a0,-1488
esp.vst.128.ip q1,a0,-1472
esp.vst.128.ip q1,a0,-1456
esp.vst.128.ip q1,a0,-1440
esp.vst.128.ip q1,a0,-1424
esp.vst.128.ip q1,a0,-1408
esp.vst.128.ip q1,a0,-1392
esp.vst.128.ip q1,a0,-1376
esp.vst.128.ip q1,a0,-1360
esp.vst.128.ip q1,a0,-1344
esp.vst.128.ip q1,a0,-1328
esp.vst.128.ip q1,a0,-1312
esp.vst.128.ip q1,a0,-1296
esp.vst.128.ip q1,a0,-1280
esp.vst.128.ip q1,a0,-1264
esp.vst.128.ip q1,a0,-1248
esp.vst.128.ip q1,a0,-1232
esp.vst.128.ip q1,a0,-1216
esp.vst.128.ip q1,a0,-1200
esp.vst.128.ip q1,a0,-1184
esp.vst.128.ip q1,a0,-1168
esp.vst.128.ip q1,a0,-1152
esp.vst.128.ip q1,a0,-1136
esp.vst.128.ip q1,a0,-1120
esp.vst.128.ip q1,a0,-1104
esp.vst.128.ip q1,a0,-1088
esp.vst.128.ip q1,a0,-1072
esp.vst.128.ip q1,a0,-1056
esp.vst.128.ip q1,a0,-1040
esp.vst.128.ip q1,a0,-1024
esp.vst.128.ip q1,a0,-1008
esp.vst.128.ip q1,a0,-992
esp.vst.128.ip q1,a0,-976
esp.vst.128.ip q1,a0,-960
esp.vst.128.ip q1,a0,-944
esp.vst.128.ip q1,a0,-928
esp.vst.128.ip q1,a0,-912
esp.vst.128.ip q1,a0,-896
esp.vst.128.ip q1,a0,-880
esp.vst.128.ip q1,a0,-864
esp.vst.128.ip q1,a0,-848
esp.vst.128.ip q1,a0,-832
esp.vst.128.ip q1,a0,-816
esp.vst.128.ip q1,a0,-800
esp.vst.128.ip q1,a0,-784
esp.vst.128.ip q1,a0,-768
esp.vst.128.ip q1,a0,-752
esp.vst.128.ip q1,a0,-736
esp.vst.128.ip q1,a0,-720
esp.vst.128.ip q1,a0,-704
esp.vst.128.ip q1,a0,-688
esp.vst.128.ip q1,a0,-672
esp.vst.128.ip q1,a0,-656
esp.vst.128.ip q1,a0,-640
esp.vst.128.ip q1,a0,-624
esp.vst.128.ip q1,a0,-608
esp.vst.128.ip q1,a0,-592
esp.vst.128.ip q1,a0,-576
esp.vst.128.ip q1,a0,-560
esp.vst.128.ip q1,a0,-544
esp.vst.128.ip q1,a0,-528
esp.vst.128.ip q1,a0,-512
esp.vst.128.ip q1,a0,-496
esp.vst.128.ip q1,a0,-480
esp.vst.128.ip q1,a0,-464
esp.vst.128.ip q1,a0,-448
esp.vst.128.ip q1,a0,-432
esp.vst.128.ip q1,a0,-416
esp.vst.128.ip q1,a0,-400
esp.vst.128.ip q1,a0,-384
esp.vst.128.ip q1,a0,-368
esp.vst.128.ip q1,a0,-352
esp.vst.128.ip q1,a0,-336
esp.vst.128.ip q1,a0,-320
esp.vst.128.ip q1,a0,-304
esp.vst.128.ip q1,a0,-288
esp.vst.128.ip q1,a0,-272
esp.vst.128.ip q1,a0,-256
esp.vst.128.ip q1,a0,-240
esp.vst.128.ip q1,a0,-224
esp.vst.128.ip q1,a0,-208
esp.vst.128.ip q1,a0,-192
esp.vst.128.ip q1,a0,-176
esp.vst.128.ip q1,a0,-160
esp.vst.128.ip q1,a0,-144
esp.vst.128.ip q1,a0,-128
esp.vst.128.ip q1,a0,-112
esp.vst.128.ip q1,a0,-96
esp.vst.128.ip q1,a0,-80
esp.vst.128.ip q1,a0,-64
esp.vst.128.ip q1,a0,-48
esp.vst.128.ip q1,a0,-32
esp.vst.128.ip q1,a0,-16
esp.vst.128.ip q1,a0,0
esp.vst.128.ip q1,a0,32
esp.vst.128.ip q1,a0,48
esp.vst.128.ip q1,a0,64
esp.vst.128.ip q1,a0,80
esp.vst.128.ip q1,a0,96
esp.vst.128.ip q1,a0,112
esp.vst.128.ip q1,a0,128
esp.vst.128.ip q1,a0,144
esp.vst.128.ip q1,a0,160
esp.vst.128.ip q1,a0,176
esp.vst.128.ip q1,a0,192
esp.vst.128.ip q1,a0,208
esp.vst.128.ip q1,a0,224
esp.vst.128.ip q1,a0,240
esp.vst.128.ip q1,a0,256
esp.vst.128.ip q1,a0,272
esp.vst.128.ip q1,a0,288
esp.vst.128.ip q1,a0,304
esp.vst.128.ip q1,a0,320
esp.vst.128.ip q1,a0,336
esp.vst.128.ip q1,a0,352
esp.vst.128.ip q1,a0,368
esp.vst.128.ip q1,a0,384
esp.vst.128.ip q1,a0,400
esp.vst.128.ip q1,a0,416
esp.vst.128.ip q1,a0,432
esp.vst.128.ip q1,a0,448
esp.vst.128.ip q1,a0,464
esp.vst.128.ip q1,a0,480
esp.vst.128.ip q1,a0,496
esp.vst.128.ip q1,a0,512
esp.vst.128.ip q1,a0,528
esp.vst.128.ip q1,a0,544
esp.vst.128.ip q1,a0,560
esp.vst.128.ip q1,a0,576
esp.vst.128.ip q1,a0,592
esp.vst.128.ip q1,a0,608
esp.vst.128.ip q1,a0,624
esp.vst.128.ip q1,a0,640
esp.vst.128.ip q1,a0,656
esp.vst.128.ip q1,a0,672
esp.vst.128.ip q1,a0,688
esp.vst.128.ip q1,a0,704
esp.vst.128.ip q1,a0,720
esp.vst.128.ip q1,a0,736
esp.vst.128.ip q1,a0,752
esp.vst.128.ip q1,a0,768
esp.vst.128.ip q1,a0,784
esp.vst.128.ip q1,a0,800
esp.vst.128.ip q1,a0,816
esp.vst.128.ip q1,a0,832
esp.vst.128.ip q1,a0,848
esp.vst.128.ip q1,a0,864
esp.vst.128.ip q1,a0,880
esp.vst.128.ip q1,a0,896
esp.vst.128.ip q1,a0,912
esp.vst.128.ip q1,a0,928
esp.vst.128.ip q1,a0,944
esp.vst.128.ip q1,a0,960
esp.vst.128.ip q1,a0,976
esp.vst.128.ip q1,a0,992
esp.vst.128.ip q1,a0,1008
esp.vst.128.ip q1,a0,1024
esp.vst.128.ip q1,a0,1040
esp.vst.128.ip q1,a0,1056
esp.vst.128.ip q1,a0,1072
esp.vst.128.ip q1,a0,1088
esp.vst.128.ip q1,a0,1104
esp.vst.128.ip q1,a0,1120
esp.vst.128.ip q1,a0,1136
esp.vst.128.ip q1,a0,1152
esp.vst.128.ip q1,a0,1168
esp.vst.128.ip q1,a0,1184
esp.vst.128.ip q1,a0,1200
esp.vst.128.ip q1,a0,1216
esp.vst.128.ip q1,a0,1232
esp.vst.128.ip q1,a0,1248
esp.vst.128.ip q1,a0,1264
esp.vst.128.ip q1,a0,1280
esp.vst.128.ip q1,a0,1296
esp.vst.128.ip q1,a0,1312
esp.vst.128.ip q1,a0,1328
esp.vst.128.ip q1,a0,1344
esp.vst.128.ip q1,a0,1360
esp.vst.128.ip q1,a0,1376
esp.vst.128.ip q1,a0,1392
esp.vst.128.ip q1,a0,1408
esp.vst.128.ip q1,a0,1424
esp.vst.128.ip q1,a0,1440
esp.vst.128.ip q1,a0,1456
esp.vst.128.ip q1,a0,1472
esp.vst.128.ip q1,a0,1488
esp.vst.128.ip q1,a0,1504
esp.vst.128.ip q1,a0,1520
esp.vst.128.ip q1,a0,1536
esp.vst.128.ip q1,a0,1552
esp.vst.128.ip q1,a0,1568
esp.vst.128.ip q1,a0,1584
esp.vst.128.ip q1,a0,1600
esp.vst.128.ip q1,a0,1616
esp.vst.128.ip q1,a0,1632
esp.vst.128.ip q1,a0,1648
esp.vst.128.ip q1,a0,1664
esp.vst.128.ip q1,a0,1680
esp.vst.128.ip q1,a0,1696
esp.vst.128.ip q1,a0,1712
esp.vst.128.ip q1,a0,1728
esp.vst.128.ip q1,a0,1744
esp.vst.128.ip q1,a0,1760
esp.vst.128.ip q1,a0,1776
esp.vst.128.ip q1,a0,1792
esp.vst.128.ip q1,a0,1808
esp.vst.128.ip q1,a0,1824
esp.vst.128.ip q1,a0,1840
esp.vst.128.ip q1,a0,1856
esp.vst.128.ip q1,a0,1872
esp.vst.128.ip q1,a0,1888
esp.vst.128.ip q1,a0,1904
esp.vst.128.ip q1,a0,1920
esp.vst.128.ip q1,a0,1936
esp.vst.128.ip q1,a0,1952
esp.vst.128.ip q1,a0,1968
esp.vst.128.ip q1,a0,1984
esp.vst.128.ip q1,a0,2000
esp.vst.128.ip q1,a0,2016
esp.vst.128.ip q1,a0,2032
esp.vldbc.16.ip q0,a0,2
esp.vldbc.16.ip q1,a0,2
esp.vldbc.16.ip q2,a0,2
esp.vldbc.16.ip q3,a0,2
esp.vldbc.16.ip q4,a0,2
esp.vldbc.16.ip q5,a0,2
esp.vldbc.16.ip q6,a0,2
esp.vldbc.16.ip q7,a0,2
esp.vldbc.16.ip q1,s0,2
esp.vldbc.16.ip q1,s1,2
esp.vldbc.16.ip q1,a1,2
esp.vldbc.16.ip q1,a2,2
esp.vldbc.16.ip q1,a3,2
esp.vldbc.16.ip q1,a4,2
esp.vldbc.16.ip q1,a5,2
esp.vldbc.16.ip q1,s8,2
esp.vldbc.16.ip q1,s9,2
esp.vldbc.16.ip q1,s10,2
esp.vldbc.16.ip q1,s11,2
esp.vldbc.16.ip q1,t3,2
esp.vldbc.16.ip q1,t4,2
esp.vldbc.16.ip q1,t5,2
esp.vldbc.16.ip q1,t6,2
esp.vldbc.16.ip q1,a0,-256
esp.vldbc.16.ip q1,a0,-254
esp.vldbc.16.ip q1,a0,-252
esp.vldbc.16.ip q1,a0,-250
esp.vldbc.16.ip q1,a0,-248
esp.vldbc.16.ip q1,a0,-246
esp.vldbc.16.ip q1,a0,-244
esp.vldbc.16.ip q1,a0,-242
esp.vldbc.16.ip q1,a0,-240
esp.vldbc.16.ip q1,a0,-238
esp.vldbc.16.ip q1,a0,-236
esp.vldbc.16.ip q1,a0,-234
esp.vldbc.16.ip q1,a0,-232
esp.vldbc.16.ip q1,a0,-230
esp.vldbc.16.ip q1,a0,-228
esp.vldbc.16.ip q1,a0,-226
esp.vldbc.16.ip q1,a0,-224
esp.vldbc.16.ip q1,a0,-222
esp.vldbc.16.ip q1,a0,-220
esp.vldbc.16.ip q1,a0,-218
esp.vldbc.16.ip q1,a0,-216
esp.vldbc.16.ip q1,a0,-214
esp.vldbc.16.ip q1,a0,-212
esp.vldbc.16.ip q1,a0,-210
esp.vldbc.16.ip q1,a0,-208
esp.vldbc.16.ip q1,a0,-206
esp.vldbc.16.ip q1,a0,-204
esp.vldbc.16.ip q1,a0,-202
esp.vldbc.16.ip q1,a0,-200
esp.vldbc.16.ip q1,a0,-198
esp.vldbc.16.ip q1,a0,-196
esp.vldbc.16.ip q1,a0,-194
esp.vldbc.16.ip q1,a0,-192
esp.vldbc.16.ip q1,a0,-190
esp.vldbc.16.ip q1,a0,-188
esp.vldbc.16.ip q1,a0,-186
esp.vldbc.16.ip q1,a0,-184
esp.vldbc.16.ip q1,a0,-182
esp.vldbc.16.ip q1,a0,-180
esp.vldbc.16.ip q1,a0,-178
esp.vldbc.16.ip q1,a0,-176
esp.vldbc.16.ip q1,a0,-174
esp.vldbc.16.ip q1,a0,-172
esp.vldbc.16.ip q1,a0,-170
esp.vldbc.16.ip q1,a0,-168
esp.vldbc.16.ip q1,a0,-166
esp.vldbc.16.ip q1,a0,-164
esp.vldbc.16.ip q1,a0,-162
esp.vldbc.16.ip q1,a0,-160
esp.vldbc.16.ip q1,a0,-158
esp.vldbc.16.ip q1,a0,-156
esp.vldbc.16.ip q1,a0,-154
esp.vldbc.16.ip q1,a0,-152
esp.vldbc.16.ip q1,a0,-150
esp.vldbc.16.ip q1,a0,-148
esp.vldbc.16.ip q1,a0,-146
esp.vldbc.16.ip q1,a0,-144
esp.vldbc.16.ip q1,a0,-142
esp.vldbc.16.ip q1,a0,-140
esp.vldbc.16.ip q1,a0,-138
esp.vldbc.16.ip q1,a0,-136
esp.vldbc.16.ip q1,a0,-134
esp.vldbc.16.ip q1,a0,-132
esp.vldbc.16.ip q1,a0,-130
esp.vldbc.16.ip q1,a0,-128
esp.vldbc.16.ip q1,a0,-126
esp.vldbc.16.ip q1,a0,-124
esp.vldbc.16.ip q1,a0,-122
esp.vldbc.16.ip q1,a0,-120
esp.vldbc.16.ip q1,a0,-118
esp.vldbc.16.ip q1,a0,-116
esp.vldbc.16.ip q1,a0,-114
esp.vldbc.16.ip q1,a0,-112
esp.vldbc.16.ip q1,a0,-110
esp.vldbc.16.ip q1,a0,-108
esp.vldbc.16.ip q1,a0,-106
esp.vldbc.16.ip q1,a0,-104
esp.vldbc.16.ip q1,a0,-102
esp.vldbc.16.ip q1,a0,-100
esp.vldbc.16.ip q1,a0,-98
esp.vldbc.16.ip q1,a0,-96
esp.vldbc.16.ip q1,a0,-94
esp.vldbc.16.ip q1,a0,-92
esp.vldbc.16.ip q1,a0,-90
esp.vldbc.16.ip q1,a0,-88
esp.vldbc.16.ip q1,a0,-86
esp.vldbc.16.ip q1,a0,-84
esp.vldbc.16.ip q1,a0,-82
esp.vldbc.16.ip q1,a0,-80
esp.vldbc.16.ip q1,a0,-78
esp.vldbc.16.ip q1,a0,-76
esp.vldbc.16.ip q1,a0,-74
esp.vldbc.16.ip q1,a0,-72
esp.vldbc.16.ip q1,a0,-70
esp.vldbc.16.ip q1,a0,-68
esp.vldbc.16.ip q1,a0,-66
esp.vldbc.16.ip q1,a0,-64
esp.vldbc.16.ip q1,a0,-62
esp.vldbc.16.ip q1,a0,-60
esp.vldbc.16.ip q1,a0,-58
esp.vldbc.16.ip q1,a0,-56
esp.vldbc.16.ip q1,a0,-54
esp.vldbc.16.ip q1,a0,-52
esp.vldbc.16.ip q1,a0,-50
esp.vldbc.16.ip q1,a0,-48
esp.vldbc.16.ip q1,a0,-46
esp.vldbc.16.ip q1,a0,-44
esp.vldbc.16.ip q1,a0,-42
esp.vldbc.16.ip q1,a0,-40
esp.vldbc.16.ip q1,a0,-38
esp.vldbc.16.ip q1,a0,-36
esp.vldbc.16.ip q1,a0,-34
esp.vldbc.16.ip q1,a0,-32
esp.vldbc.16.ip q1,a0,-30
esp.vldbc.16.ip q1,a0,-28
esp.vldbc.16.ip q1,a0,-26
esp.vldbc.16.ip q1,a0,-24
esp.vldbc.16.ip q1,a0,-22
esp.vldbc.16.ip q1,a0,-20
esp.vldbc.16.ip q1,a0,-18
esp.vldbc.16.ip q1,a0,-16
esp.vldbc.16.ip q1,a0,-14
esp.vldbc.16.ip q1,a0,-12
esp.vldbc.16.ip q1,a0,-10
esp.vldbc.16.ip q1,a0,-8
esp.vldbc.16.ip q1,a0,-6
esp.vldbc.16.ip q1,a0,-4
esp.vldbc.16.ip q1,a0,-2
esp.vldbc.16.ip q1,a0,0
esp.vldbc.16.ip q1,a0,4
esp.vldbc.16.ip q1,a0,6
esp.vldbc.16.ip q1,a0,8
esp.vldbc.16.ip q1,a0,10
esp.vldbc.16.ip q1,a0,12
esp.vldbc.16.ip q1,a0,14
esp.vldbc.16.ip q1,a0,16
esp.vldbc.16.ip q1,a0,18
esp.vldbc.16.ip q1,a0,20
esp.vldbc.16.ip q1,a0,22
esp.vldbc.16.ip q1,a0,24
esp.vldbc.16.ip q1,a0,26
esp.vldbc.16.ip q1,a0,28
esp.vldbc.16.ip q1,a0,30
esp.vldbc.16.ip q1,a0,32
esp.vldbc.16.ip q1,a0,34
esp.vldbc.16.ip q1,a0,36
esp.vldbc.16.ip q1,a0,38
esp.vldbc.16.ip q1,a0,40
esp.vldbc.16.ip q1,a0,42
esp.vldbc.16.ip q1,a0,44
esp.vldbc.16.ip q1,a0,46
esp.vldbc.16.ip q1,a0,48
esp.vldbc.16.ip q1,a0,50
esp.vldbc.16.ip q1,a0,52
esp.vldbc.16.ip q1,a0,54
esp.vldbc.16.ip q1,a0,56
esp.vldbc.16.ip q1,a0,58
esp.vldbc.16.ip q1,a0,60
esp.vldbc.16.ip q1,a0,62
esp.vldbc.16.ip q1,a0,64
esp.vldbc.16.ip q1,a0,66
esp.vldbc.16.ip q1,a0,68
esp.vldbc.16.ip q1,a0,70
esp.vldbc.16.ip q1,a0,72
esp.vldbc.16.ip q1,a0,74
esp.vldbc.16.ip q1,a0,76
esp.vldbc.16.ip q1,a0,78
esp.vldbc.16.ip q1,a0,80
esp.vldbc.16.ip q1,a0,82
esp.vldbc.16.ip q1,a0,84
esp.vldbc.16.ip q1,a0,86
esp.vldbc.16.ip q1,a0,88
esp.vldbc.16.ip q1,a0,90
esp.vldbc.16.ip q1,a0,92
esp.vldbc.16.ip q1,a0,94
esp.vldbc.16.ip q1,a0,96
esp.vldbc.16.ip q1,a0,98
esp.vldbc.16.ip q1,a0,100
esp.vldbc.16.ip q1,a0,102
esp.vldbc.16.ip q1,a0,104
esp.vldbc.16.ip q1,a0,106
esp.vldbc.16.ip q1,a0,108
esp.vldbc.16.ip q1,a0,110
esp.vldbc.16.ip q1,a0,112
esp.vldbc.16.ip q1,a0,114
esp.vldbc.16.ip q1,a0,116
esp.vldbc.16.ip q1,a0,118
esp.vldbc.16.ip q1,a0,120
esp.vldbc.16.ip q1,a0,122
esp.vldbc.16.ip q1,a0,124
esp.vldbc.16.ip q1,a0,126
esp.vldbc.16.ip q1,a0,128
esp.vldbc.16.ip q1,a0,130
esp.vldbc.16.ip q1,a0,132
esp.vldbc.16.ip q1,a0,134
esp.vldbc.16.ip q1,a0,136
esp.vldbc.16.ip q1,a0,138
esp.vldbc.16.ip q1,a0,140
esp.vldbc.16.ip q1,a0,142
esp.vldbc.16.ip q1,a0,144
esp.vldbc.16.ip q1,a0,146
esp.vldbc.16.ip q1,a0,148
esp.vldbc.16.ip q1,a0,150
esp.vldbc.16.ip q1,a0,152
esp.vldbc.16.ip q1,a0,154
esp.vldbc.16.ip q1,a0,156
esp.vldbc.16.ip q1,a0,158
esp.vldbc.16.ip q1,a0,160
esp.vldbc.16.ip q1,a0,162
esp.vldbc.16.ip q1,a0,164
esp.vldbc.16.ip q1,a0,166
esp.vldbc.16.ip q1,a0,168
esp.vldbc.16.ip q1,a0,170
esp.vldbc.16.ip q1,a0,172
esp.vldbc.16.ip q1,a0,174
esp.vldbc.16.ip q1,a0,176
esp.vldbc.16.ip q1,a0,178
esp.vldbc.16.ip q1,a0,180
esp.vldbc.16.ip q1,a0,182
esp.vldbc.16.ip q1,a0,184
esp.vldbc.16.ip q1,a0,186
esp.vldbc.16.ip q1,a0,188
esp.vldbc.16.ip q1,a0,190
esp.vldbc.16.ip q1,a0,192
esp.vldbc.16.ip q1,a0,194
esp.vldbc.16.ip q1,a0,196
esp.vldbc.16.ip q1,a0,198
esp.vldbc.16.ip q1,a0,200
esp.vldbc.16.ip q1,a0,202
esp.vldbc.16.ip q1,a0,204
esp.vldbc.16.ip q1,a0,206
esp.vldbc.16.ip q1,a0,208
esp.vldbc.16.ip q1,a0,210
esp.vldbc.16.ip q1,a0,212
esp.vldbc.16.ip q1,a0,214
esp.vldbc.16.ip q1,a0,216
esp.vldbc.16.ip q1,a0,218
esp.vldbc.16.ip q1,a0,220
esp.vldbc.16.ip q1,a0,222
esp.vldbc.16.ip q1,a0,224
esp.vldbc.16.ip q1,a0,226
esp.vldbc.16.ip q1,a0,228
esp.vldbc.16.ip q1,a0,230
esp.vldbc.16.ip q1,a0,232
esp.vldbc.16.ip q1,a0,234
esp.vldbc.16.ip q1,a0,236
esp.vldbc.16.ip q1,a0,238
esp.vldbc.16.ip q1,a0,240
esp.vldbc.16.ip q1,a0,242
esp.vldbc.16.ip q1,a0,244
esp.vldbc.16.ip q1,a0,246
esp.vldbc.16.ip q1,a0,248
esp.vldbc.16.ip q1,a0,250
esp.vldbc.16.ip q1,a0,252
esp.vldbc.16.ip q1,a0,254
esp.ld.128.usar.ip q0,a0,16
esp.ld.128.usar.ip q1,a0,16
esp.ld.128.usar.ip q2,a0,16
esp.ld.128.usar.ip q3,a0,16
esp.ld.128.usar.ip q4,a0,16
esp.ld.128.usar.ip q5,a0,16
esp.ld.128.usar.ip q6,a0,16
esp.ld.128.usar.ip q7,a0,16
esp.ld.128.usar.ip q1,s0,16
esp.ld.128.usar.ip q1,s1,16
esp.ld.128.usar.ip q1,a1,16
esp.ld.128.usar.ip q1,a2,16
esp.ld.128.usar.ip q1,a3,16
esp.ld.128.usar.ip q1,a4,16
esp.ld.128.usar.ip q1,a5,16
esp.ld.128.usar.ip q1,s8,16
esp.ld.128.usar.ip q1,s9,16
esp.ld.128.usar.ip q1,s10,16
esp.ld.128.usar.ip q1,s11,16
esp.ld.128.usar.ip q1,t3,16
esp.ld.128.usar.ip q1,t4,16
esp.ld.128.usar.ip q1,t5,16
esp.ld.128.usar.ip q1,t6,16
esp.ld.128.usar.ip q1,a0,-2048
esp.ld.128.usar.ip q1,a0,-2032
esp.ld.128.usar.ip q1,a0,-2016
esp.ld.128.usar.ip q1,a0,-2000
esp.ld.128.usar.ip q1,a0,-1984
esp.ld.128.usar.ip q1,a0,-1968
esp.ld.128.usar.ip q1,a0,-1952
esp.ld.128.usar.ip q1,a0,-1936
esp.ld.128.usar.ip q1,a0,-1920
esp.ld.128.usar.ip q1,a0,-1904
esp.ld.128.usar.ip q1,a0,-1888
esp.ld.128.usar.ip q1,a0,-1872
esp.ld.128.usar.ip q1,a0,-1856
esp.ld.128.usar.ip q1,a0,-1840
esp.ld.128.usar.ip q1,a0,-1824
esp.ld.128.usar.ip q1,a0,-1808
esp.ld.128.usar.ip q1,a0,-1792
esp.ld.128.usar.ip q1,a0,-1776
esp.ld.128.usar.ip q1,a0,-1760
esp.ld.128.usar.ip q1,a0,-1744
esp.ld.128.usar.ip q1,a0,-1728
esp.ld.128.usar.ip q1,a0,-1712
esp.ld.128.usar.ip q1,a0,-1696
esp.ld.128.usar.ip q1,a0,-1680
esp.ld.128.usar.ip q1,a0,-1664
esp.ld.128.usar.ip q1,a0,-1648
esp.ld.128.usar.ip q1,a0,-1632
esp.ld.128.usar.ip q1,a0,-1616
esp.ld.128.usar.ip q1,a0,-1600
esp.ld.128.usar.ip q1,a0,-1584
esp.ld.128.usar.ip q1,a0,-1568
esp.ld.128.usar.ip q1,a0,-1552
esp.ld.128.usar.ip q1,a0,-1536
esp.ld.128.usar.ip q1,a0,-1520
esp.ld.128.usar.ip q1,a0,-1504
esp.ld.128.usar.ip q1,a0,-1488
esp.ld.128.usar.ip q1,a0,-1472
esp.ld.128.usar.ip q1,a0,-1456
esp.ld.128.usar.ip q1,a0,-1440
esp.ld.128.usar.ip q1,a0,-1424
esp.ld.128.usar.ip q1,a0,-1408
esp.ld.128.usar.ip q1,a0,-1392
esp.ld.128.usar.ip q1,a0,-1376
esp.ld.128.usar.ip q1,a0,-1360
esp.ld.128.usar.ip q1,a0,-1344
esp.ld.128.usar.ip q1,a0,-1328
esp.ld.128.usar.ip q1,a0,-1312
esp.ld.128.usar.ip q1,a0,-1296
esp.ld.128.usar.ip q1,a0,-1280
esp.ld.128.usar.ip q1,a0,-1264
esp.ld.128.usar.ip q1,a0,-1248
esp.ld.128.usar.ip q1,a0,-1232
esp.ld.128.usar.ip q1,a0,-1216
esp.ld.128.usar.ip q1,a0,-1200
esp.ld.128.usar.ip q1,a0,-1184
esp.ld.128.usar.ip q1,a0,-1168
esp.ld.128.usar.ip q1,a0,-1152
esp.ld.128.usar.ip q1,a0,-1136
esp.ld.128.usar.ip q1,a0,-1120
esp.ld.128.usar.ip q1,a0,-1104
esp.ld.128.usar.ip q1,a0,-1088
esp.ld.128.usar.ip q1,a0,-1072
esp.ld.128.usar.ip q1,a0,-1056
esp.ld.128.usar.ip q1,a0,-1040
esp.ld.128.usar.ip q1,a0,-1024
esp.ld.128.usar.ip q1,a0,-1008
esp.ld.128.usar.ip q1,a0,-992
esp.ld.128.usar.ip q1,a0,-976
esp.ld.128.usar.ip q1,a0,-960
esp.ld.128.usar.ip q1,a0,-944
esp.ld.128.usar.ip q1,a0,-928
esp.ld.128.usar.ip q1,a0,-912
esp.ld.128.usar.ip q1,a0,-896
esp.ld.128.usar.ip q1,a0,-880
esp.ld.128.usar.ip q1,a0,-864
esp.ld.128.usar.ip q1,a0,-848
esp.ld.128.usar.ip q1,a0,-832
esp.ld.128.usar.ip q1,a0,-816
esp.ld.128.usar.ip q1,a0,-800
esp.ld.128.usar.ip q1,a0,-784
esp.ld.128.usar.ip q1,a0,-768
esp.ld.128.usar.ip q1,a0,-752
esp.ld.128.usar.ip q1,a0,-736
esp.ld.128.usar.ip q1,a0,-720
esp.ld.128.usar.ip q1,a0,-704
esp.ld.128.usar.ip q1,a0,-688
esp.ld.128.usar.ip q1,a0,-672
esp.ld.128.usar.ip q1,a0,-656
esp.ld.128.usar.ip q1,a0,-640
esp.ld.128.usar.ip q1,a0,-624
esp.ld.128.usar.ip q1,a0,-608
esp.ld.128.usar.ip q1,a0,-592
esp.ld.128.usar.ip q1,a0,-576
esp.ld.128.usar.ip q1,a0,-560
esp.ld.128.usar.ip q1,a0,-544
esp.ld.128.usar.ip q1,a0,-528
esp.ld.128.usar.ip q1,a0,-512
esp.ld.128.usar.ip q1,a0,-496
esp.ld.128.usar.ip q1,a0,-480
esp.ld.128.usar.ip q1,a0,-464
esp.ld.128.usar.ip q1,a0,-448
esp.ld.128.usar.ip q1,a0,-432
esp.ld.128.usar.ip q1,a0,-416
esp.ld.128.usar.ip q1,a0,-400
esp.ld.128.usar.ip q1,a0,-384
esp.ld.128.usar.ip q1,a0,-368
esp.ld.128.usar.ip q1,a0,-352
esp.ld.128.usar.ip q1,a0,-336
esp.ld.128.usar.ip q1,a0,-320
esp.ld.128.usar.ip q1,a0,-304
esp.ld.128.usar.ip q1,a0,-288
esp.ld.128.usar.ip q1,a0,-272
esp.ld.128.usar.ip q1,a0,-256
esp.ld.128.usar.ip q1,a0,-240
esp.ld.128.usar.ip q1,a0,-224
esp.ld.128.usar.ip q1,a0,-208
esp.ld.128.usar.ip q1,a0,-192
esp.ld.128.usar.ip q1,a0,-176
esp.ld.128.usar.ip q1,a0,-160
esp.ld.128.usar.ip q1,a0,-144
esp.ld.128.usar.ip q1,a0,-128
esp.ld.128.usar.ip q1,a0,-112
esp.ld.128.usar.ip q1,a0,-96
esp.ld.128.usar.ip q1,a0,-80
esp.ld.128.usar.ip q1,a0,-64
esp.ld.128.usar.ip q1,a0,-48
esp.ld.128.usar.ip q1,a0,-32
esp.ld.128.usar.ip q1,a0,-16
esp.ld.128.usar.ip q1,a0,0
esp.ld.128.usar.ip q1,a0,32
esp.ld.128.usar.ip q1,a0,48
esp.ld.128.usar.ip q1,a0,64
esp.ld.128.usar.ip q1,a0,80
esp.ld.128.usar.ip q1,a0,96
esp.ld.128.usar.ip q1,a0,112
esp.ld.128.usar.ip q1,a0,128
esp.ld.128.usar.ip q1,a0,144
esp.ld.128.usar.ip q1,a0,160
esp.ld.128.usar.ip q1,a0,176
esp.ld.128.usar.ip q1,a0,192
esp.ld.128.usar.ip q1,a0,208
esp.ld.128.usar.ip q1,a0,224
esp.ld.128.usar.ip q1,a0,240
esp.ld.128.usar.ip q1,a0,256
esp.ld.128.usar.ip q1,a0,272
esp.ld.128.usar.ip q1,a0,288
esp.ld.128.usar.ip q1,a0,304
esp.ld.128.usar.ip q1,a0,320
esp.ld.128.usar.ip q1,a0,336
esp.ld.128.usar.ip q1,a0,352
esp.ld.128.usar.ip q1,a0,368
esp.ld.128.usar.ip q1,a0,384
esp.ld.128.usar.ip q1,a0,400
esp.ld.128.usar.ip q1,a0,416
esp.ld.128.usar.ip q1,a0,432
esp.ld.128.usar.ip q1,a0,448
esp.ld.128.usar.ip q1,a0,464
esp.ld.128.usar.ip q1,a0,480
esp.ld.128.usar.ip q1,a0,496
esp.ld.128.usar.ip q1,a0,512
esp.ld.128.usar.ip q1,a0,528
esp.ld.128.usar.ip q1,a0,544
esp.ld.128.usar.ip q1,a0,560
esp.ld.128.usar.ip q1,a0,576
esp.ld.128.usar.ip q1,a0,592
esp.ld.128.usar.ip q1,a0,608
esp.ld.128.usar.ip q1,a0,624
esp.ld.128.usar.ip q1,a0,640
esp.ld.128.usar.ip q1,a0,656
esp.ld.128.usar.ip q1,a0,672
esp.ld.128.usar.ip q1,a0,688
esp.ld.128.usar.ip q1,a0,704
esp.ld.128.usar.ip q1,a0,720
esp.ld.128.usar.ip q1,a0,736
esp.ld.128.usar.ip q1,a0,752
esp.ld.128.usar.ip q1,a0,768
esp.ld.128.usar.ip q1,a0,784
esp.ld.128.usar.ip q1,a0,800
esp.ld.128.usar.ip q1,a0,816
esp.ld.128.usar.ip q1,a0,832
esp.ld.128.usar.ip q1,a0,848
esp.ld.128.usar.ip q1,a0,864
esp.ld.128.usar.ip q1,a0,880
esp.ld.128.usar.ip q1,a0,896
esp.ld.128.usar.ip q1,a0,912
esp.ld.128.usar.ip q1,a0,928
esp.ld.128.usar.ip q1,a0,944
esp.ld.128.usar.ip q1,a0,960
esp.ld.128.usar.ip q1,a0,976
esp.ld.128.usar.ip q1,a0,992
esp.ld.128.usar.ip q1,a0,1008
esp.ld.128.usar.ip q1,a0,1024
esp.ld.128.usar.ip q1,a0,1040
esp.ld.128.usar.ip q1,a0,1056
esp.ld.128.usar.ip q1,a0,1072
esp.ld.128.usar.ip q1,a0,1088
esp.ld.128.usar.ip q1,a0,1104
esp.ld.128.usar.ip q1,a0,1120
esp.ld.128.usar.ip q1,a0,1136
esp.ld.128.usar.ip q1,a0,1152
esp.ld.128.usar.ip q1,a0,1168
esp.ld.128.usar.ip q1,a0,1184
esp.ld.128.usar.ip q1,a0,1200
esp.ld.128.usar.ip q1,a0,1216
esp.ld.128.usar.ip q1,a0,1232
esp.ld.128.usar.ip q1,a0,1248
esp.ld.128.usar.ip q1,a0,1264
esp.ld.128.usar.ip q1,a0,1280
esp.ld.128.usar.ip q1,a0,1296
esp.ld.128.usar.ip q1,a0,1312
esp.ld.128.usar.ip q1,a0,1328
esp.ld.128.usar.ip q1,a0,1344
esp.ld.128.usar.ip q1,a0,1360
esp.ld.128.usar.ip q1,a0,1376
esp.ld.128.usar.ip q1,a0,1392
esp.ld.128.usar.ip q1,a0,1408
esp.ld.128.usar.ip q1,a0,1424
esp.ld.128.usar.ip q1,a0,1440
esp.ld.128.usar.ip q1,a0,1456
esp.ld.128.usar.ip q1,a0,1472
esp.ld.128.usar.ip q1,a0,1488
esp.ld.128.usar.ip q1,a0,1504
esp.ld.128.usar.ip q1,a0,1520
esp.ld.128.usar.ip q1,a0,1536
esp.ld.128.usar.ip q1,a0,1552
esp.ld.128.usar.ip q1,a0,1568
esp.ld.128.usar.ip q1,a0,1584
esp.ld.128.usar.ip q1,a0,1600
esp.ld.128.usar.ip q1,a0,1616
esp.ld.128.usar.ip q1,a0,1632
esp.ld.128.usar.ip q1,a0,1648
esp.ld.128.usar.ip q1,a0,1664
esp.ld.128.usar.ip q1,a0,1680
esp.ld.128.usar.ip q1,a0,1696
esp.ld.128.usar.ip q1,a0,1712
esp.ld.128.usar.ip q1,a0,1728
esp.ld.128.usar.ip q1,a0,1744
esp.ld.128.usar.ip q1,a0,1760
esp.ld.128.usar.ip q1,a0,1776
esp.ld.128.usar.ip q1,a0,1792
esp.ld.128.usar.ip q1,a0,1808
esp.ld.128.usar.ip q1,a0,1824
esp.ld.128.usar.ip q1,a0,1840
esp.ld.128.usar.ip q1,a0,1856
esp.ld.128.usar.ip q1,a0,1872
esp.ld.128.usar.ip q1,a0,1888
esp.ld.128.usar.ip q1,a0,1904
esp.ld.128.usar.ip q1,a0,1920
esp.ld.128.usar.ip q1,a0,1936
esp.ld.128.usar.ip q1,a0,1952
esp.ld.128.usar.ip q1,a0,1968
esp.ld.128.usar.ip q1,a0,1984
esp.ld.128.usar.ip q1,a0,2000
esp.ld.128.usar.ip q1,a0,2016
esp.ld.128.usar.ip q1,a0,2032
esp.movx.w.sar s0
esp.movx.w.sar s1
esp.movx.w.sar a0
esp.movx.w.sar a1
esp.movx.w.sar a2
esp.movx.w.sar a3
esp.movx.w.sar a4
esp.movx.w.sar a5
esp.movx.w.sar s8
esp.movx.w.sar s9
esp.movx.w.sar s10
esp.movx.w.sar s11
esp.movx.w.sar t3
esp.movx.w.sar t4
esp.movx.w.sar t5
esp.movx.w.sar t6
esp.movx.r.sar s0
esp.movx.r.sar s1
esp.movx.r.sar a0
esp.movx.r.sar a1
esp.movx.r.sar a2
esp.movx.r.sar a3
esp.movx.r.sar a4
esp.movx.r.sar a5
esp.movx.r.sar s8
esp.movx.r.sar s9
esp.movx.r.sar s10
esp.movx.r.sar s11
esp.movx.r.sar t3
esp.movx.r.sar t4
esp.movx.r.sar t5
esp.movx.r.sar t6
esp.movx.w.cfg s0
esp.movx.w.cfg s1
esp.movx.w.cfg a0
esp.movx.w.cfg a1
esp.movx.w.cfg a2
esp.movx.w.cfg a3
esp.movx.w.cfg a4
esp.movx.w.cfg a5
esp.movx.w.cfg s8
esp.movx.w.cfg s9
esp.movx.w.cfg s10
esp.movx.w.cfg s11
esp.movx.w.cfg t3
esp.movx.w.cfg t4
esp.movx.w.cfg t5
esp.movx.w.cfg t6
esp.movx.r.cfg s0
esp.movx.r.cfg s1
esp.movx.r.cfg a0
esp.movx.r.cfg a1
esp.movx.r.cfg a2
esp.movx.r.cfg a3
esp.movx.r.cfg a4
esp.movx.r.cfg a5
esp.movx.r.cfg s8
esp.movx.r.cfg s9
esp.movx.r.cfg s10
esp.movx.r.cfg s11
esp.movx.r.cfg t3
esp.movx.r.cfg t4
esp.movx.r.cfg t5
esp.movx.r.cfg t6
esp.movi.32.a q0,a0,1
esp.movi.32.a q1,a0,1
esp.movi.32.a q2,a0,1
esp.movi.32.a q3,a0,1
esp.movi.32.a q4,a0,1
esp.movi.32.a q5,a0,1
esp.movi.32.a q6,a0,1
esp.movi.32.a q7,a0,1
esp.movi.32.a q1,s0,1
esp.movi.32.a q1,s1,1
esp.movi.32.a q1,a1,1
esp.movi.32.a q1,a2,1
esp.movi.32.a q1,a3,1
esp.movi.32.a q1,a4,1
esp.movi.32.a q1,a5,1
esp.movi.32.a q1,s8,1
esp.movi.32.a q1,s9,1
esp.movi.32.a q1,s10,1
esp.movi.32.a q1,s11,1
esp.movi.32.a q1,t3,1
esp.movi.32.a q1,t4,1
esp.movi.32.a q1,t5,1
esp.movi.32.a q1,t6,1
esp.movi.32.a q1,a0,0
esp.movi.32.a q1,a0,2
esp.movi.32.a q1,a0,3
.insn 4, 0x0000001b
.insn 4, 0x0000001f
.insn 4, 0x0000141b
.insn 4, 0x0000181b
.insn 4, 0x0000201b
.insn 4, 0x0000205f
.insn 4, 0x0000211b
.insn 4, 0x0000251b
.insn 4, 0x0000301b
.insn 4, 0x0000401b
.insn 4, 0x0000411f
.insn 4, 0x0000501b
.insn 4, 0x0000901b
.insn 4, 0x0001101b
.insn 4, 0x0002101b
.insn 4, 0x0004001b
.insn 4, 0x0004001f
.insn 4, 0x0004101b
.insn 4, 0x0004241b
.insn 4, 0x0004281b
.insn 4, 0x0004301b
.insn 4, 0x0004601b
.insn 4, 0x0004a01b
.insn 4, 0x0005201b
.insn 4, 0x0006201b
.insn 4, 0x0008101b
.insn 4, 0x0010101b
.insn 4, 0x0020101b
.insn 4, 0x0020201b
.insn 4, 0x0040011b
.insn 4, 0x0040051b
.insn 4, 0x0040101b
.insn 4, 0x0040201b
.insn 4, 0x0040219b
.insn 4, 0x0040231b
.insn 4, 0x0040241b
.insn 4, 0x0040259b
.insn 4, 0x0040271b
.insn 4, 0x0040291b
.insn 4, 0x00402d1b
.insn 4, 0x0040311b
.insn 4, 0x0040351b
.insn 4, 0x0040611b
.insn 4, 0x0040651b
.insn 4, 0x0040a11b
.insn 4, 0x0040a51b
.insn 4, 0x0041211b
.insn 4, 0x0041251b
.insn 4, 0x0042211b
.insn 4, 0x0042251b
.insn 4, 0x0044211b
.insn 4, 0x0044251b
.insn 4, 0x0050211b
.insn 4, 0x0050251b
.insn 4, 0x0060211b
.insn 4, 0x0060251b
.insn 4, 0x0080001f
.insn 4, 0x0080101b
.insn 4, 0x0080201b
.insn 4, 0x0080271b
.insn 4, 0x0084201b
.insn 4, 0x0084231b
.insn 4, 0x0084271b
.insn 4, 0x00a0001b
.insn 4, 0x00a0281b
.insn 4, 0x00a0301b
.insn 4, 0x00a0601b
.insn 4, 0x00a0a01b
.insn 4, 0x00a1201b
.insn 4, 0x00a2201b
.insn 4, 0x00a8201b
.insn 4, 0x00b0201b
.insn 4, 0x00c0201b
.insn 4, 0x00c0211b
.insn 4, 0x00c0251b
.insn 4, 0x0100005f
.insn 4, 0x0100011f
.insn 4, 0x0100041f
.insn 4, 0x0100081f
.insn 4, 0x0100101b
.insn 4, 0x0100101f
.insn 4, 0x0100201f
.insn 4, 0x0100401f
.insn 4, 0x0100419f
.insn 4, 0x0100431f
.insn 4, 0x0100605f
.insn 4, 0x0100611f
.insn 4, 0x0100801f
.insn 4, 0x0101001f
.insn 4, 0x0102001f
.insn 4, 0x0104041f
.insn 4, 0x0104081f
.insn 4, 0x0104101f
.insn 4, 0x0104201f
.insn 4, 0x0104401f
.insn 4, 0x0104801f
.insn 4, 0x0105001f
.insn 4, 0x0106001f
.insn 4, 0x0108001f
.insn 4, 0x010c001f
.insn 4, 0x0110001f
.insn 4, 0x0114001f
.insn 4, 0x0120001f
.insn 4, 0x0124001f
.insn 4, 0x0140001f
.insn 4, 0x0141001f
.insn 4, 0x0144001f
.insn 4, 0x0144981f
.insn 4, 0x0180041f
.insn 4, 0x0180081f
.insn 4, 0x0180101f
.insn 4, 0x0180201f
.insn 4, 0x0180205f
.insn 4, 0x0180401f
.insn 4, 0x0180411f
.insn 4, 0x0180801f
.insn 4, 0x0181001f
.insn 4, 0x0182001f
.insn 4, 0x0184001f
.insn 4, 0x0188001f
.insn 4, 0x0190001f
.insn 4, 0x01a0001f
.insn 4, 0x01a0201b
.insn 4, 0x01c0001f
.insn 4, 0x01c4001f
.insn 4, 0x0200101b
.insn 4, 0x0200271b
.insn 4, 0x0204201b
.insn 4, 0x0204231b
.insn 4, 0x0204271b
.insn 4, 0x0210001f
.insn 4, 0x0241001f
.insn 4, 0x0244981f
.insn 4, 0x0280071b
.insn 4, 0x0280201b
.insn 4, 0x0280231b
.insn 4, 0x0280251b
.insn 4, 0x0280261b
.insn 4, 0x0280279b
.insn 4, 0x02802f1b
.insn 4, 0x0280371b
.insn 4, 0x0280671b
.insn 4, 0x0280a71b
.insn 4, 0x0282271b
.insn 4, 0x0284001b
.insn 4, 0x0284031b
.insn 4, 0x0284071b
.insn 4, 0x0284209b
.insn 4, 0x0284211b
.insn 4, 0x0284221b
.insn 4, 0x0284239b
.insn 4, 0x0284241b
.insn 4, 0x0284251b
.insn 4, 0x0284261b
.insn 4, 0x0284279b
.insn 4, 0x0284281b
.insn 4, 0x02842b1b
.insn 4, 0x02842f1b
.insn 4, 0x0284301b
.insn 4, 0x0284331b
.insn 4, 0x0284371b
.insn 4, 0x0284601b
.insn 4, 0x0284631b
.insn 4, 0x0284671b
.insn 4, 0x0284a01b
.insn 4, 0x0284a31b
.insn 4, 0x0284a71b
.insn 4, 0x0285201b
.insn 4, 0x0286201b
.insn 4, 0x0286231b
.insn 4, 0x0286271b
.insn 4, 0x0288271b
.insn 4, 0x028c201b
.insn 4, 0x028c231b
.insn 4, 0x028c271b
.insn 4, 0x0290001f
.insn 4, 0x0294201b
.insn 4, 0x02a0001f
.insn 4, 0x02a0201b
.insn 4, 0x02a4201b
.insn 4, 0x02c4001f
.insn 4, 0x02c4201b
.insn 4, 0x0300001f
.insn 4, 0x0300205f
.insn 4, 0x0300411f
.insn 4, 0x0301001f
.insn 4, 0x0304001f
.insn 4, 0x0304981f
.insn 4, 0x0310201f
.insn 4, 0x0310401f
.insn 4, 0x0318001f
.insn 4, 0x0320001f
.insn 4, 0x0330001f
.insn 4, 0x0340001f
.insn 4, 0x0340981f
.insn 4, 0x0341041f
.insn 4, 0x0341081f
.insn 4, 0x0341101f
.insn 4, 0x0341201f
.insn 4, 0x0341401f
.insn 4, 0x0341801f
.insn 4, 0x0343001f
.insn 4, 0x0344001f
.insn 4, 0x0344181f
.insn 4, 0x0344881f
.insn 4, 0x0344901f
.insn 4, 0x03449c1f
.insn 4, 0x0344b81f
.insn 4, 0x0344d81f
.insn 4, 0x0345001f
.insn 4, 0x0345981f
.insn 4, 0x0346981f
.insn 4, 0x0349001f
.insn 4, 0x034c981f
.insn 4, 0x0350001f
.insn 4, 0x0351001f
.insn 4, 0x0354981f
.insn 4, 0x0361001f
.insn 4, 0x0364981f
.insn 4, 0x0380001f
.insn 4, 0x0380271b
.insn 4, 0x0384001f
.insn 4, 0x0384201b
.insn 4, 0x0384231b
.insn 4, 0x0384271b
.insn 4, 0x0390009f
.insn 4, 0x0390201f
.insn 4, 0x0390401f
.insn 4, 0x0398001f
.insn 4, 0x03a0009f
.insn 4, 0x03a0201f
.insn 4, 0x03a0401f
.insn 4, 0x03a8001f
.insn 4, 0x03b0001f
.insn 4, 0x03c0001f
.insn 4, 0x03c1001f
.insn 4, 0x03c4041f
.insn 4, 0x03c4201f
.insn 4, 0x03c4401f
.insn 4, 0x03c4981f
.insn 4, 0x03c6001f
.insn 4, 0x03cc001f
.insn 4, 0x03d0001f
.insn 4, 0x03d4001f
.insn 4, 0x03e0001f
.insn 4, 0x03e4001f
.insn 4, 0x0400101b
.insn 4, 0x0404201b
.insn 4, 0x0800101b
.insn 4, 0x0804201b
.insn 4, 0x1000101b
.insn 4, 0x1004201b
.insn 4, 0x1080201b
.insn 4, 0x10c0201b
.insn 4, 0x2000101b
.insn 4, 0x2004201b
.insn 4, 0x20a0201b
.insn 4, 0x4000101b
.insn 4, 0x4004201b
.insn 4, 0x40a0201b
.insn 4, 0x8000001b
.insn 4, 0x8000101b
.insn 4, 0x8000201b
.insn 4, 0x8000441b
.insn 4, 0x8000481b
.insn 4, 0x8000501b
.insn 4, 0x8000601b
.insn 4, 0x8000c01b
.insn 4, 0x8001401b
.insn 4, 0x8002401b
.insn 4, 0x8004201b
.insn 4, 0x8004401b
.insn 4, 0x8010401b
.insn 4, 0x8020401b
.insn 4, 0x8040201b
.insn 4, 0x8040401b
.insn 4, 0x8080001b
.insn 4, 0x8080281b
.insn 4, 0x8080301b
.insn 4, 0x8080401b
.insn 4, 0x8080601b
.insn 4, 0x8080a01b
.insn 4, 0x8081201b
.insn 4, 0x8082201b
.insn 4, 0x8084201b
.insn 4, 0x8088201b
.insn 4, 0x8090201b
.insn 4, 0x80a0201b
.insn 4, 0x80c0001b
.insn 4, 0x80c0281b
.insn 4, 0x80c0301b
.insn 4, 0x80c0601b
.insn 4, 0x80c0a01b
.insn 4, 0x80c1201b
.insn 4, 0x80c2201b
.insn 4, 0x80c4201b
.insn 4, 0x80c8201b
.insn 4, 0x80d0201b
.insn 4, 0x80e0201b
.insn 4, 0x8110001f
.insn 4, 0x8180201b
.insn 4, 0x81c0201b
.insn 4, 0x8210001f
.insn 4, 0x8280201b
.insn 4, 0x82c0201b
.insn 4, 0x8300001f
.insn 4, 0x8310201f
.insn 4, 0x8310401f
.insn 4, 0x8318001f
.insn 4, 0x8330001f
.insn 4, 0x8350001f
.insn 4, 0x8390001f
.insn 4, 0x8480201b
.insn 4, 0x84c0201b
.insn 4, 0x8880201b
.insn 4, 0x88c0201b
.insn 4, 0x9000201b
.insn 4, 0x9040201b
.insn 4, 0x9080001b
.insn 4, 0x9080209b
.insn 4, 0x9080211b
.insn 4, 0x9080221b
.insn 4, 0x9080241b
.insn 4, 0x9080281b
.insn 4, 0x9080301b
.insn 4, 0x9080601b
.insn 4, 0x9088201b
.insn 4, 0x9090201b
.insn 4, 0x90a0201b
.insn 4, 0x90c0001b
.insn 4, 0x90c0209b
.insn 4, 0x90c0211b
.insn 4, 0x90c0221b
.insn 4, 0x90c0241b
.insn 4, 0x90c0281b
.insn 4, 0x90c0301b
.insn 4, 0x90c0601b
.insn 4, 0x90c8201b
.insn 4, 0x90d0201b
.insn 4, 0x90e0201b
.insn 4, 0x9180201b
.insn 4, 0x91c0201b
.insn 4, 0x9280201b
.insn 4, 0x92c0201b
.insn 4, 0x9480201b
.insn 4, 0x94c0201b
.insn 4, 0x9880201b
.insn 4, 0x98c0201b
.insn 4, 0xa000401b
.insn 4, 0xa080201b
.insn 4, 0xa0c0201b
.insn 4, 0xb080201b
.insn 4, 0xb0c0201b
.insn 4, 0xc000401b
.insn 4, 0xc080201b
.insn 4, 0xc0c0201b
.insn 4, 0xd080201b
.insn 4, 0xd0c0201b

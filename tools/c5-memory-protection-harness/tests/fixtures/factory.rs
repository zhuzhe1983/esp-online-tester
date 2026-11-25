// Static CSR values from original LCD273 cf10a435... RV32 disassembly and IDF553
// 2c211b236707889e8400c4dc5644dd5c4ee071e0 cpu_region_protect.c.
// This replays CSR requests, not firmware instructions. No original binary is embedded.
use c5_memory_protection_harness::policy::*;

pub const PMA_ADDR: [u32; 16] = [0x03ffffff,0x0c000000,0x10000000,0x10000000,
    0x10014000,0x10218000,0x10800000,0x10bfffff,0x11000000,0x14000000,
    0x18040000,0x3fffffff,0x10014000,0x10200000,0x14001000,0x18000000];
pub const PMA_CFG: [u32; 16] = [0xe0000001,0x20000001,0x60000001,0x20000001,
    0x60000015,0x20000001,0x60000001,0xe000001d,0x20000001,0x60000001,
    0x20000001,0x60000001,0x20000001,0x60000001,0x20000001,0x60000001];

#[derive(Clone, Copy)]
enum Op { Write(u32,u32), Set(u32,u32), Clear(u32,u32) }
fn apply(p: &mut C5MemoryProtection, ctx: GuardedMachineContext, op: Op) {
    let (csr,value) = match op {
        Op::Write(c,v) => (c,v),
        Op::Set(c,v) => (c,p.read_csr(c,ctx).unwrap() | v),
        Op::Clear(c,v) => (c,p.read_csr(c,ctx).unwrap() & !v),
    };
    p.write_csr(csr,value,ctx).unwrap();
}
pub fn pma(p: &mut C5MemoryProtection, ctx: GuardedMachineContext) -> usize {
    // All64 writes at420713ba..42071512: OFF, zero address, real address, LOCK.
    for i in 0..16 {
        for op in [Op::Write(0xbc0+i,0),Op::Write(0xbd0+i,0),
            Op::Write(0xbd0+i,PMA_ADDR[i as usize]),Op::Write(0xbc0+i,PMA_CFG[i as usize])] { apply(p,ctx,op); }
    }
    64
}
pub fn pmp(p: &mut C5MemoryProtection, ctx: GuardedMachineContext, heap: u32) {
    // Explicit test parameter H, not an observed guest return. These are the
    // original non-OCD branch's requests in execution order, with its ROM
    // drom_start40049c00 and original linker boundaries40818400/421228f4/4230105c.
    let end = (0x4231_0000u32.checked_add(heap).unwrap() & !127) >> 2;
    use Op::*;
    let ops = [
        Write(0x3b0,0x09ffffff),Set(0x3a0,0x9f),
        Write(0x3b1,0x10012700),Set(0x3a0,0x8000),Write(0x3b2,0x10014000),Set(0x3a0,0x00890000),
        Clear(0x3a0,0xff000000),Clear(0x3a1,0xff),Clear(0x3a1,0xff00),
        Write(0x3b3,0x10200000),Set(0x3a0,0x80000000),Write(0x3b4,0x10206100),Set(0x3a1,0x8d),
        Write(0x3b5,0x10218000),Set(0x3a1,0x8b00),
        Clear(0x3a1,0x00ff0000),Write(0x3b6,0x10800000),Set(0x3a1,0x00800000),
        Clear(0x3a1,0xff000000),Clear(0x3a2,0xff),Clear(0x3a2,0xff00),
        Write(0x3b7,0x1084c000),Set(0x3a1,0x8d000000),Write(0x3b8,0x1084c000),Set(0x3a2,0x80),
        Write(0x3b9,0x108c4000),Set(0x3a2,0x8900),Clear(0x3a2,0x00ff0000),Write(0x3ba,end),Set(0x3a2,0x008b0000),
        Clear(0x3a2,0xff000000),Clear(0x3a3,0xff),Clear(0x3a3,0xff00),Clear(0x3a3,0x00ff0000),
        Write(0x3bb,0x14000000),Set(0x3a2,0x80000000),Write(0x3bc,0x14000000),Set(0x3a3,0x8b),
        Write(0x3bd,0x14000000),Set(0x3a3,0x8d00),Write(0x3be,0x14001000),Set(0x3a3,0x008b0000),
        Clear(0x3a3,0xff000000),Write(0x3bf,0x1801ffff),Set(0x3a3,0x9b000000),
    ];
    for op in ops { apply(p,ctx,op); }
}

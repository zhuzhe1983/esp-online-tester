//! Ordinary scalar accesses in the validated-ROM profile. C5 TRM1.1
//!2.12.1.2/6.3.3.1; no split MMIO/cross-page or physical timing claims.
use crate::{C5Bus,Machine};
use emu_core::{Bus,Fault};
use esp_soc::SocBus;
const RAM:u32=crate::bus::SRAM_BASE;
fn selected()->C5Bus{let mut b=C5Bus::new();b.select_validated_rom_profile().unwrap();b.rom_boot=true;b}
struct UnitCost;
impl emu_core::CostModel for UnitCost{
    fn lifecycle(&mut self,_:&emu_core::LifecycleFacts)->Result<(),String>{Ok(())}
    fn cycles(&mut self,_:&emu_core::ExecutionFacts<'_>)->Result<u32,String>{Ok(1)}
}
fn guest(words:&[u32],priced:bool)->Machine{
    let mut m=crate::rom_machine([0;6],4<<20);m.quantum=1;
    m.bus.select_validated_rom_profile().unwrap();m.bus.rom_boot=true;m.cores[0].enable_c5_memory_protection().unwrap();
    let bytes=words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>();m.bus.load_ram(RAM,&bytes,bytes.len()).unwrap();
    m.cores[0].pc=RAM;m.cores[0].x[2]=RAM+0x50000;
    if priced{m.set_cost_model(Box::new(UnitCost)).unwrap();}m
}
#[test]fn ordinary_internal_reads_cover_every_halfword_and_word_offset(){
    let bytes:[u8;12]=[0,1,2,3,4,5,6,7,8,9,10,11];
    for base in[RAM,crate::bus::LP_BASE,crate::bus::ROM_BASE]{
        let mut b=C5Bus::new();
        // Synthetic backing patterns, not a ROM-image/lifecycle validation.
        match base{RAM=>b.sram[..12].copy_from_slice(&bytes),crate::bus::LP_BASE=>b.lp_sram[..12].copy_from_slice(&bytes),_=>b.rom[..12].copy_from_slice(&bytes)}
        b.select_validated_rom_profile().unwrap();b.rom_boot=true;
        for off in 0..8{let i=off as usize;
            assert_eq!(b.read16(base+off),Ok(u16::from_le_bytes([bytes[i],bytes[i+1]])));
            assert_eq!(b.read32(base+off),Ok(u32::from_le_bytes(bytes[i..i+4].try_into().unwrap())));
        }
    }
}
#[test]fn ordinary_internal_stores_preserve_neighbor_bytes_and_little_endian_order(){
    for base in[RAM,crate::bus::LP_BASE]{for off in[1,2,3]{
        let mut b=selected();b.load_bytes(base,&[0xa5;12]).unwrap();b.write32(base+off,0x7856_3412).unwrap();
        for i in 0..12{let expected=if (off..off+4).contains(&i){[0x12,0x34,0x56,0x78][(i-off)as usize]}else{0xa5};assert_eq!(b.read8(base+i),Ok(expected));}
        b.write16(base+off,0x9abc).unwrap();assert_eq!(b.read8(base+off),Ok(0xbc));assert_eq!(b.read8(base+off+1),Ok(0x9a));
        assert_eq!(b.read8(base+off+2),Ok(0x56));assert_eq!(b.read8(base+off+3),Ok(0x78));
    }}
}
#[test]fn gcc15_packed_image_revision_loads_execute_without_guest_or_data_patches(){
    // Actual IDF6.0.2 bootloader instructions: lhu s2,15(s1); lhu s1,17(s1).
    for priced in[false,true]{let mut m=guest(&[0x00f4_d903,0x0114_d483],priced);let data=RAM+0x1000;
        let mut header=[0u8;24];header[15..17].copy_from_slice(&100u16.to_le_bytes());header[17..19].copy_from_slice(&199u16.to_le_bytes());
        m.bus.load_bytes(data,&header).unwrap();m.cores[0].x[9]=data;
        for _ in 0..2{assert!(matches!(m.run(1),esp_soc::Stop::MaxInsns));}
        assert_eq!((m.cores[0].x[18],m.cores[0].x[9]),(100,199));assert_eq!(m.cores[0].pc,RAM+8);
        assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());
    }
}
#[test]fn unaligned_load_sign_extension_and_store_use_real_guest_memory(){
    // lh a0,1(t0); lw a1,3(t0); sh a2,5(t0); sw a3,7(t0).
    for priced in[false,true]{let mut m=guest(&[0x0012_9503,0x0032_a583,0x00c2_92a3,0x00d2_a3a3],priced);let data=RAM+0x1000;
        m.bus.load_bytes(data,&[0,0,0x80,0x78,0x56,0x34,0x12,0,0,0,0,0]).unwrap();m.cores[0].x[5]=data;
        m.cores[0].x[12]=0xabcd;m.cores[0].x[13]=0xfedc_ba98;
        for _ in 0..4{assert!(matches!(m.run(1),esp_soc::Stop::MaxInsns));}
        assert_eq!(m.cores[0].x[10],0xffff_8000);assert_eq!(m.cores[0].x[11],0x1234_5678);
        assert_eq!(m.bus.read16(data+5),Ok(0xabcd));assert_eq!(m.bus.read32(data+7),Ok(0xfedc_ba98));assert_eq!(m.exceptions,0);
    }
}
#[test]fn unaligned_store_still_checks_the_whole_pmp_span_before_changing_memory(){
    for priced in[false,true]{let mut m=guest(&[0x00b2_a023],priced);let data=RAM+0x1000;
        m.bus.load_bytes(data,&[0xa5;132]).unwrap();m.cores[0].x[5]=data+127;m.cores[0].x[11]=0x1234_5678;
        m.cores[0].try_write_csr(0x3b0,(data>>2)|15).unwrap();m.cores[0].try_write_csr(0x3a0,0x9b).unwrap();
        assert!(matches!(m.run(1),esp_soc::Stop::Exceptions(1)));assert_eq!(m.cores[0].mcause,7);assert_eq!(m.cores[0].mtval,data+127);
        for i in 124..132{assert_eq!(m.bus.read8(data+i),Ok(0xa5));}
    }
}
#[test]fn unknown_pma_on_an_unaligned_access_remains_a_model_limit(){
    for priced in[false,true]{let mut m=guest(&[0x0012_a503],priced);let data=RAM+0x1000;
        m.bus.load_bytes(data,&[0x5a;8]).unwrap();m.cores[0].x[5]=data;m.cores[0].x[10]=0x55;
        m.cores[0].try_write_csr(0xbd0,data>>2).unwrap();m.cores[0].mepc=17;m.cores[0].mcause=19;m.cores[0].mtval=23;
        assert!(matches!(m.run(1),esp_soc::Stop::ModelLimit{core:0,pc:RAM}));assert_eq!(m.cores[0].x[10],0x55);
        assert_eq!((m.cores[0].mepc,m.cores[0].mcause,m.cores[0].mtval),(17,19,23));assert_eq!(m.exceptions,0);
    }
}
#[test]fn unmodeled_split_domains_are_rejected_before_any_device_or_partial_store_effect(){
    let mut b=selected();let uart=b.uart[0].tx_out.clone();
    for a in[0x6000_0001,0x6009_6165,0x4200_ffff,0xffff_ffff]{
        assert_eq!(b.read16(a),Err(Fault::Unsupported));assert_eq!(b.write32(a,0x1234_5678),Err(Fault::Unsupported));
    }
    for(base,len)in[(RAM,crate::bus::SRAM_SIZE),(crate::bus::LP_BASE,crate::bus::LP_SIZE)]{
        let tail=base+len as u32-1;b.write8(tail,0x5a).unwrap();
        assert_eq!(b.write32(tail,0x1234_5678),Err(Fault::Unsupported));assert_eq!(b.read8(tail),Ok(0x5a));
    }
    assert_eq!(b.uart[0].tx_out,uart);
}
#[test]fn instruction_atomic_and_unselected_paths_keep_their_alignment_guards(){
    let mut b=selected();b.load_bytes(RAM,&[0x13,0,0x13,0,0,0]).unwrap();
    assert_eq!(b.fetch(RAM+1),Err(Fault::Misaligned));assert_eq!(b.fetch(RAM+2).unwrap(),[0x13,0,0,0]);
    for(read,write)in[(true,false),(false,true),(true,true)]{assert_eq!(b.check_atomic32(RAM+1,read,write),Err(Fault::Misaligned));}
    b.deselect_validated_rom_profile();assert_eq!(b.read16(RAM+1),Err(Fault::Misaligned));assert_eq!(b.write32(RAM+1,0),Err(Fault::Misaligned));
    let mut legacy=C5Bus::new();assert_eq!(legacy.read16(RAM+1),Err(Fault::Misaligned));assert_eq!(legacy.write32(RAM+1,0),Err(Fault::Misaligned));
}
const MAPPED:u32=0x4200_0000;
fn put(b:&mut C5Bus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn cached(b:&mut C5Bus,ram:bool){
    // Same IDF5.5.3/5.5.4 controller/pad/QPI sequence as tests/psram.rs,
    // including a real Quad die command; a valid MMU word alone is insufficient.
    if ram{
        b.configure_psram(8<<20).unwrap();for pin in[15,17,18,20,21,22]{put(b,0x6009_0000+4*pin,0x800);}
        for(a,v)in[(0x6000_3034,1),(0x6000_3008,0),(0x6000_3018,1<<31),(0x6000_3020,(7<<28)|0x35),(0x6000_3000,1<<18),
            (0x6000_2040,(1<<20)|(23<<14)|(5<<6)|0x34),(0x6000_2048,(7<<28)|0xeb),(0x6000_204c,(7<<28)|0x38)]{put(b,a,v);}
    }else{b.flash=vec![0xa5;2<<16];}
    for(a,v)in[(0x6000_203c,0),(0x6000_2384,4),(0x6000_2380,0),(0x6000_237c,if ram{0x600}else{0x400})]{put(b,a,v);}
}
#[test]fn cached_scalar_loads_and_psram_stores_cover_offsets_without_neighbor_changes(){
    for ram in[false,true]{let mut b=selected();cached(&mut b,ram);let bytes:[u8;12]=[0,1,2,3,4,5,6,7,8,9,10,11];
        if ram{b.psram[..12].copy_from_slice(&bytes);}else{b.flash[..12].copy_from_slice(&bytes);}
        for off in 0..8{let i=off as usize;assert_eq!(b.read16(MAPPED+off),Ok(u16::from_le_bytes([bytes[i],bytes[i+1]])));
            assert_eq!(b.read32(MAPPED+off),Ok(u32::from_le_bytes(bytes[i..i+4].try_into().unwrap())));}
        for off in[1,2,3]{
            if ram{b.psram[..12].fill(0xa5);b.write32(MAPPED+off,0x7856_3412).unwrap();
                for i in 0..12{assert_eq!(b.psram[i],if (off as usize..off as usize+4).contains(&i){[0x12,0x34,0x56,0x78][i-off as usize]}else{0xa5});}
                b.write16(MAPPED+off,0x9abc).unwrap();assert_eq!(&b.psram[off as usize..off as usize+4],&[0xbc,0x9a,0x56,0x78]);
            }else{let before=b.flash.clone();assert_eq!(b.write32(MAPPED+off,0x1234_5678),Err(Fault::Unsupported));assert_eq!(b.flash,before);}
        }
    }
}
#[test]fn original_jpeg_header_lhu_executes_at_an_odd_cached_address(){
    for priced in[false,true]{let mut m=guest(&[0x0027_5783],priced);cached(&mut m.bus,true);
        m.bus.psram[..8].copy_from_slice(&[0xff,0xdb,0x00,0x43,0x12,0x80,0x56,0]);m.cores[0].x[14]=MAPPED+1;
        assert!(matches!(m.run(1),esp_soc::Stop::MaxInsns));assert_eq!(m.cores[0].x[15],0x1243);assert_eq!(m.cores[0].pc,RAM+4);
        assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());
    }
}
#[test]fn cached_span_refuses_cross_page_invalid_encrypted_and_capacity_paths(){
    for selector in 0..4{let mut b=selected();cached(&mut b,true);let page=65536usize>>selector;
        put(&mut b,0x6000_2384,4|selector<<3);put(&mut b,0x6000_2380,1);put(&mut b,0x6000_237c,0x601);
        b.psram[page-4..page+4].fill(0xa5);let address=MAPPED+page as u32-1;
        assert_eq!(b.read16(address),Err(Fault::Unsupported));assert_eq!(b.write32(address,0x1234_5678),Err(Fault::Unsupported));
        assert_eq!(&b.psram[page-4..page+4],&[0xa5;8],"even contiguous next pages are outside the split contract");
        b.psram[1..5].copy_from_slice(&[1,2,3,4]);assert_eq!(b.read32(MAPPED+1),Ok(0x0403_0201));
    }
    for entry in[0x200,0xe00,0x680]{let mut b=selected();cached(&mut b,true);b.psram[..8].fill(0xa5);
        put(&mut b,0x6000_237c,entry);assert_eq!(b.read16(MAPPED+1),Err(Fault::Unsupported));
        assert_eq!(b.write32(MAPPED+1,0x1234_5678),Err(Fault::Unsupported));assert_eq!(&b.psram[..8],&[0xa5;8]);
    }
}
#[test]fn cached_unaligned_accesses_keep_cache_mspi_and_quad_die_gates(){
    for gate in 0..8{let mut b=selected();cached(&mut b,true);b.psram[..8].fill(0xa5);
        match gate{
            0=>put(&mut b,0x600c_8004,2),1=>put(&mut b,0x6009_601c,0x500),
            2=>put(&mut b,0x6009_0048,0x1800),3=>{put(&mut b,0x6000_2048,(7<<28)|3);put(&mut b,0x6000_204c,(7<<28)|2);},
            4=>put(&mut b,0x6000_2384,6),5=>put(&mut b,0x6000_203c,1<<31),6=>put(&mut b,0x6000_2040,0),
            _=>{for(a,v)in[(0x6000_3008,(1<<8)|(1<<13)|(1<<24)),(0x6000_3018,(1<<31)|(1<<15)),(0x6000_3020,(7<<28)|0xf5),(0x6000_3000,1<<18)]{put(&mut b,a,v);}}
        }
        assert!(b.read16(MAPPED+1).is_err(),"read gate{gate}");assert!(b.write32(MAPPED+1,0x1234_5678).is_err(),"write gate{gate}");
        assert_eq!(&b.psram[..8],&[0xa5;8],"gate{gate} cannot partly write");
    }
}
#[test]fn cached_unaligned_stores_check_whole_pmp_and_pma_before_any_byte_changes(){
    for priced in[false,true]{for pma in[false,true]{let mut m=guest(&[0x00b2_a023],priced);cached(&mut m.bus,true);m.bus.psram[..132].fill(0xa5);
        let address=MAPPED+127;m.cores[0].x[5]=address;m.cores[0].x[11]=0x1234_5678;
        if pma{m.cores[0].try_write_csr(0xbd0,(MAPPED>>2)|0x1ff).unwrap();m.cores[0].try_write_csr(0xbc0,0xc000_0015).unwrap();}
        else{m.cores[0].try_write_csr(0x3b0,(MAPPED>>2)|15).unwrap();m.cores[0].try_write_csr(0x3a0,0x9b).unwrap();}
        assert!(matches!(m.run(1),esp_soc::Stop::Exceptions(1)));assert_eq!((m.cores[0].mcause,m.cores[0].mtval),(7,address));
        assert_eq!(&m.bus.psram[..132],&[0xa5;132]);let fault=m.bus.first_fault.as_ref().unwrap();assert_eq!((fault.address,fault.width),(address,4));
    }}
}
#[test]fn cached_atomic_fetch_unselected_and_unconstrained_context_guards_stay_intact(){
    let mut b=selected();cached(&mut b,true);b.psram[..8].fill(0xa5);
    assert_eq!(b.fetch(MAPPED+1),Err(Fault::Misaligned));
    for(read,write)in[(true,false),(false,true),(true,true)]{assert_eq!(b.check_atomic32(MAPPED+1,read,write),Err(Fault::Misaligned));}
    let mut cpu=riscv_rv32::Cpu::new();cpu.machine_only=false;let monitor=b.cpu_monitor();monitor.borrow_mut().begin_core(RAM,&cpu);
    assert_eq!(b.write32(MAPPED+1,0x1234_5678),Err(Fault::Prohibited));assert_eq!(&b.psram[..8],&[0xa5;8]);monitor.borrow_mut().end();
    b.deselect_validated_rom_profile();assert_eq!(b.read16(MAPPED+1),Err(Fault::Misaligned));assert_eq!(b.write32(MAPPED+1,0),Err(Fault::Misaligned));
}

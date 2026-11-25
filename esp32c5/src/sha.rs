//! C5 CPU-block SHA-1/224/256 with actual linked PCR reset/gate state.
//! ESP-IDF v5.5.3/v5.5.4 sha_reg.h/sha_ll.h and C5 TRM1.1 chapter26.
//! Compression is synchronous and real; no DMA, crypto-key engines, padding,
//! host image acceptance or physical hash-clock latency is supplied here.
pub(crate) struct C5Sha{
    core:esp_periph::Sha,conf:u32,linked:[u32;3],dma_blocks:u32,date:u32,
}
impl C5Sha{
    pub fn new()->Self{Self{core:esp_periph::Sha::new(),conf:0,linked:[0;3],dma_blocks:0,date:539_232_291}}
    fn reset_held(&self)->bool{self.conf&2!=0||self.linked.iter().any(|v|v&2!=0)}
    fn enabled(&self)->bool{self.conf&1!=0&&!self.reset_held()}
    fn reset_engine(&mut self){self.core=esp_periph::Sha::new();self.dma_blocks=0;self.date=539_232_291;}
    fn linked_index(address:u32)->Option<usize>{match address{0x6009_60e4=>Some(0),0x6009_60e8=>Some(1),0x6009_60ec=>Some(2),_=>None}}
    pub fn read(&mut self,address:u32)->Option<u32>{
        if address==0x6009_60d0{return Some(self.conf|if self.reset_held(){0}else{4});}
        if let Some(n)=Self::linked_index(address){return Some(self.linked[n]|if self.linked[n]&2==0{4}else{0});}
        let off=address.checked_sub(0x6008_9000)?;if !off.is_multiple_of(4)||!self.enabled(){return None;}
        Some(match off{
            0=>self.core.mode,0x0c=>self.dma_blocks,0x18=>u32::from(self.core.busy),0x28=>0,0x2c=>self.date,
            // C5 TRM26.5 lists eight H words and sixteen M words. The upper
            // generic512/debug windows in generated headers are not enabled.
            0x40..=0x5c|0x80..=0xbc=>self.core.read(off),_=>return None,
        })
    }
    pub fn write(&mut self,address:u32,value:u32)->bool{
        if address==0x6009_60d0{
            self.conf=value&3;if self.reset_held(){self.reset_engine();}return true;
        }
        if let Some(n)=Self::linked_index(address){
            // These are implemented SHA reset inputs only. Do not enable or
            // claim DS/HMAC/ECDSA engines, keys or their functional clocks.
            if value&1!=0{return false;}self.linked[n]=value&2;if self.reset_held(){self.reset_engine();}return true;
        }
        let Some(off)=address.checked_sub(0x6008_9000)else{return false;};if !off.is_multiple_of(4)||!self.enabled(){return false;}
        match off{
            0=>{let mode=value&15;if mode>2{return false;}self.core.mode=mode;true}
            0x0c=>{self.dma_blocks=value&65535;true} // C5 header16bits; shared older engines use6.
            0x10|0x14=>{
                if value&!1!=0{return false;}
                if value==0{return true;}
                // START supplies the selected FIPS initial state; CONTINUE
                // uses the actual H buffer, including an interleaved restore.
                // BUSY returns idle only after this compression has run.
                let blocks=self.core.blocks;self.core.blocks=0;self.core.busy=true;
                self.core.write(off,1);self.core.busy=false;self.core.blocks=blocks.saturating_add(1);true
            }
            0x18=>true, // RO; stores cannot manufacture an engine transition.
            0x28=>value==0, // DMA IRQ stays disabled; enabling it is unsupported.
            0x2c=>{self.date=value&0x3fff_ffff;true}
            0x40..=0x5c|0x80..=0xbc=>{self.core.write(off,value);true}
            // Debug T registers, DMA commands/FIFO, IRQ operations and all
            // other crypto engines retain a strict unsupported boundary.
            _=>false,
        }
    }
}

#[cfg(test)]
mod tests{
    use super::C5Sha;
    const SHA:u32=0x6008_9000;const PCR:u32=0x6009_60d0;
    // NIST CAVP shabytetestvectors.zip, SHA{1,224,256}ShortMsg.rsp:
    // lengths0,24,440,448,512bits. Fixed independent answers, not this engine.
    const VECTORS:&[(u32,&str,&str)]=&[
        (0,"","da39a3ee5e6b4b0d3255bfef95601890afd80709"),
        (0,"df4bd2","bf36ed5d74727dfd5d7854ec6b1d49468d8ee8aa"),
        (0,"ec6b4a88713df27c0f2d02e738b69db43abda3921317259c864c1c386e9a5a3f533dc05f3beeb2bec2aac8e06db4c6cb3cddcf697e03d5","a7272e2308622ff7a339460adc61efd0ea8dabdc"),
        (0,"0321736beba578e90abc1a90aa56157d871618f6de0d764cc8c91e06c68ecd3b9de3824064503384db67beb7fe012232dacaef93a000fba7","aef843b86916c16f66c84d83a6005d23fd005c9e"),
        (0,"45927e32ddf801caf35e18e7b5078b7f5435278212ec6bb99df884f49b327c6486feae46ba187dc1cc9145121e1492e6b06e9007394dc33b7748f86ac3207cfe","a70cfbfe7563dd0e665c7c6715a96a8d756950c0"),
        (1,"","d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f"),
        (1,"51ca3d","2c8959023515476e38388abb43599a29876b4b33d56adc06032de3a2"),
        (1,"445e8698eeb8accbaac4ffa7d934fffd16014a430ef70f3a9174c6cfe96d1e3f6ab1377f4a7212dbb30146dd17d9f470c4dffc45b8e871","4c7ae028c0fe61f2a9cada61fae30685b77f04c6442576e912af9fa6"),
        (1,"52839f2f0853a30df14ec897a1914c685c1ac21470d00654c8c37663bfb65fa732dbb694d9dd09ced723b48d8f545846ba168988b61cc724","2f755a57674b49d5c25cb37348f35b6fd2de2552c749f2645ba63d20"),
        (1,"a3310ba064be2e14ad32276e18cd0310c933a6e650c3c754d0243c6c61207865b4b65248f66a08edf6e0832689a9dc3a2e5d2095eeea50bd862bac88c8bd318d","b2a5586d9cbf0baa999157b4af06d88ae08d7c9faab4bc1a96829d65"),
        (2,"","e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
        (2,"b4190e","dff2e73091f6c05e528896c4c831b9448653dc2ff043528f6769437bc7b975c2"),
        (2,"3ebfb06db8c38d5ba037f1363e118550aad94606e26835a01af05078533cc25f2f39573c04b632f62f68c294ab31f2a3e2a1a0d8c2be51","6595a2ef537a69ba8583dfbf7f5bec0ab1f93ce4c8ee1916eff44a93af5749c4"),
        (2,"2d52447d1244d2ebc28650e7b05654bad35b3a68eedc7f8515306b496d75f3e73385dd1b002625024b81a02f2fd6dffb6e6d561cb7d0bd7a","cfb88d6faf2de3a69d36195acec2e255e2af2b7d933997f348e09f6ce5758360"),
        (2,"5a86b737eaea8ee976a0a24da63e7ed7eefad18a101c1211e2b3650c5187c2a8a650547208251f6d4237e661c7bf4c77f335390394c37fa1a9f9be836ac28509","42e61e174fbb3897d6dd6cef3dd2802fe67b331953b06114a65c772859dfc1aa"),
    ];
    fn put(s:&mut C5Sha,a:u32,v:u32){assert!(s.write(a,v),"write {a:#x}={v:#x}");}
    fn unhex(s:&str)->Vec<u8>{s.as_bytes().as_chunks::<2>().0.iter().map(|v|u8::from_str_radix(std::str::from_utf8(v).unwrap(),16).unwrap()).collect()}
    fn padded(message:&[u8])->Vec<u8>{let mut b=message.to_vec();b.push(128);while b.len()%64!=56{b.push(0);}b.extend_from_slice(&(message.len() as u64*8).to_be_bytes());b}
    fn block(s:&mut C5Sha,b:&[u8],first:bool){for(i,w)in b.as_chunks::<4>().0.iter().enumerate(){put(s,SHA+0x80+i as u32*4,u32::from_le_bytes(*w));}put(s,SHA+if first{0x10}else{0x14},1);assert_eq!(s.read(SHA+0x18),Some(0));}
    fn digest(s:&mut C5Sha,len:usize)->String{let mut b=Vec::new();for i in 0..len/4{b.extend_from_slice(&s.read(SHA+0x40+i as u32*4).unwrap().to_le_bytes());}b.iter().map(|v|format!("{v:02x}")).collect()}
    fn hash(s:&mut C5Sha,mode:u32,message:&[u8],len:usize)->String{put(s,SHA,mode);for(i,b)in padded(message).as_chunks::<64>().0.iter().enumerate(){block(s,b,i==0);}digest(s,len)}
    #[test]
    fn nist_cpu_register_vectors_cover_modes_endian_and_two_block_padding(){
        let mut s=C5Sha::new();put(&mut s,PCR,1);
        for &(mode,msg,expected)in VECTORS{assert_eq!(hash(&mut s,mode,&unhex(msg),expected.len()/2),expected,"mode{mode} bytes{}",msg.len()/2);}
    }
    #[test]
    fn interleaved_h_state_restore_and_continue_use_actual_guest_words(){
        let mut s=C5Sha::new();put(&mut s,PCR,1);
        for &(mode,msg,expected)in VECTORS.iter().filter(|v|v.1.len()==128){
            let p=padded(&unhex(msg));put(&mut s,SHA,mode);block(&mut s,&p[..64],true);
            let h:[u32;8]=std::array::from_fn(|i|s.read(SHA+0x40+i as u32*4).unwrap());
            hash(&mut s,(mode+1)%3,b"an interleaved message",20);
            put(&mut s,SHA,mode);for(i,v)in h.into_iter().enumerate(){put(&mut s,SHA+0x40+i as u32*4,v);}
            block(&mut s,&p[64..],false);assert_eq!(digest(&mut s,expected.len()/2),expected);
        }
    }
    #[test]
    fn real_gate_and_linked_resets_control_readiness_and_state(){
        let mut s=C5Sha::new();assert_eq!(s.read(PCR),Some(4));assert_eq!(s.read(SHA),None);assert!(!s.write(SHA,2));put(&mut s,PCR,5);
        put(&mut s,SHA+0x40,0x1234_5678);put(&mut s,SHA+0x80,0x8877_6655);put(&mut s,SHA+0x2c,0x1234);
        put(&mut s,PCR,0);assert_eq!(s.read(SHA+0x40),None);put(&mut s,PCR,1);assert_eq!(s.read(SHA+0x40),Some(0x1234_5678));
        for a in[0x6009_60e4,0x6009_60e8,0x6009_60ec]{
            assert_eq!(s.read(a),Some(4));assert!(!s.write(a,1));assert_eq!(s.read(SHA+0x40),Some(0x1234_5678));
            put(&mut s,a,2);assert_eq!(s.read(PCR),Some(1));assert_eq!(s.read(a),Some(2));assert_eq!(s.read(SHA),None);assert!(!s.write(SHA+0x10,1));
            put(&mut s,a,0);assert_eq!(s.read(PCR),Some(5));assert_eq!(s.read(SHA),Some(2));assert_eq!(s.read(SHA+0x40),Some(0));assert_eq!(s.read(SHA+0x80),Some(0));assert_eq!(s.read(SHA+0x2c),Some(539_232_291));
            put(&mut s,SHA+0x40,0x1234_5678);
        }
        put(&mut s,PCR,3);assert_eq!(s.read(PCR),Some(3));put(&mut s,PCR,1);assert_eq!(s.read(SHA+0x40),Some(0));
    }
    #[test]
    fn unsupported_modes_dma_and_debug_do_not_hash_or_mutate(){
        let mut s=C5Sha::new();put(&mut s,PCR,1);put(&mut s,SHA+0x40,0x1122_3344);put(&mut s,SHA+0x80,0x5566_7788);
        for mode in 3..16{assert!(!s.write(SHA,mode));assert_eq!(s.read(SHA),Some(2));}
        for off in[4,8,0x1c,0x20,0x24,0x30,0x34,0x60,0x7c,0xc0,0xfc,0x100]{assert!(!s.write(SHA+off,1));assert_eq!(s.read(SHA+off),None);}
        for off in[0x10,0x14]{for value in[2,u32::MAX]{assert!(!s.write(SHA+off,value));}put(&mut s,SHA+off,0);assert_eq!(s.read(SHA+off),None);}
        assert!(!s.write(SHA+0x28,1));assert_eq!(s.read(SHA+0x28),Some(0));assert!(!s.write(SHA+0x81,1));assert_eq!(s.read(SHA+0x81),None);
        assert_eq!(s.core.blocks,0);assert_eq!(s.read(SHA+0x40),Some(0x1122_3344));assert_eq!(s.read(SHA+0x80),Some(0x5566_7788));
        put(&mut s,SHA+0x0c,u32::MAX);assert_eq!(s.read(SHA+0x0c),Some(65535));put(&mut s,SHA+0x2c,u32::MAX);assert_eq!(s.read(SHA+0x2c),Some(0x3fff_ffff));
    }
    #[test]
    fn original_rom_enable_start_continue_and_disable_contract(){
        let mut s=C5Sha::new();let p=s.read(PCR).unwrap();put(&mut s,PCR,p|1);
        for a in[0x6009_60e8,0x6009_60e4,0x6009_60ec]{let p=s.read(a).unwrap();put(&mut s,a,p&!2);}
        let p=s.read(PCR).unwrap();put(&mut s,PCR,p&!2);assert_eq!(s.read(PCR).unwrap()&4,4);
        assert_eq!(hash(&mut s,2,b"abc",32),"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(s.core.blocks,1);put(&mut s,SHA+0x18,1);for _ in 0..3{assert_eq!(s.read(SHA+0x18),Some(0));}assert_eq!(s.core.blocks,1);
        let p=s.read(PCR).unwrap();put(&mut s,PCR,p|2);let p=s.read(PCR).unwrap();put(&mut s,PCR,p&!1);assert_eq!(s.read(PCR),Some(2));assert_eq!(s.read(SHA),None);
    }
}

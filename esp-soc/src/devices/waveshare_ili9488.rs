//! ILI9488 DBI Type C subset for C5 LCD2.73. V100 data sheet §§4.3,4.4,
//! 5.2.24,5.2.30,5.2.34,6.1: complete bytes survive CS pauses; three 6-bit
//! components make a pixel; RAMWR is bounded to its window; GRAM is320x480.
//! This is not the legacy ST7789's16-bit/RAMCTRL parser.
pub struct Ili9488 {
    pub gram: Vec<u32>, // R6:G6:B6, full18-bit storage; host export is RGB565.
    pub madctl:u8, pub colmod:u8, pub inverted:bool, pub sleeping:bool, pub on:bool,
    command:u8, args:[u8;4], argn:usize, pixel:[u8;3], pixeln:usize,
    x0:u16,x1:u16,y0:u16,y1:u16,x:u16,y:u16,remaining:u32,
    pub frames:u64, pub pixels_written:u64, pub resets:u64,
}
impl Default for Ili9488 { fn default()->Self { Self::new() } }
impl Ili9488 {
    pub fn new()->Self {
        Self { gram:vec![0;320*480],madctl:0,colmod:6,inverted:false,sleeping:true,on:false,
            command:0,args:[0;4],argn:0,pixel:[0;3],pixeln:0,
            x0:0,x1:319,y0:0,y1:479,x:0,y:0,remaining:0,frames:0,pixels_written:0,resets:0 }
    }
    pub fn reset(&mut self) {
        // The PDF declares memory contents undefined after reset. Deterministic
        // zero is this virtual scene's placeholder; no retention is promised.
        let (frames,pixels,resets)=(self.frames,self.pixels_written,self.resets+1);
        *self=Self::new();self.frames=frames;self.pixels_written=pixels;self.resets=resets;
    }
    pub fn byte(&mut self,dc:bool,value:u8)->bool {
        if !dc {
            self.command=value;self.argn=0;self.pixeln=0;
            match value {
                1=>self.reset(),0x10=>self.sleeping=true,0x11=>self.sleeping=false,
                0x20=>self.inverted=false,0x21=>self.inverted=true,0x28=>self.on=false,0x29=>self.on=true,
                0x2c=>{self.x=self.x0;self.y=self.y0;self.frames+=1;
                    self.remaining=if self.x1>=self.x0&&self.y1>=self.y0 { (u32::from(self.x1-self.x0)+1).saturating_mul(u32::from(self.y1-self.y0)+1) } else { 0 };}
                _=>{}, // Analog initialization has no modeled pixel effects.
            }
            return true;
        }
        match self.command {
            0x2a|0x2b=>{
                if self.argn>=4{return false;}self.args[self.argn]=value;self.argn+=1;
                if self.argn==4 {
                    let start=u16::from_be_bytes([self.args[0],self.args[1]]);let end=u16::from_be_bytes([self.args[2],self.args[3]]);
                    if self.command==0x2a { self.x0=start;self.x1=end; } else { self.y0=start;self.y1=end; }
                }
            }
            0x36=>self.madctl=value,0x3a=>self.colmod=value,
            0x2c=>{
                // Source BSP uses MV0. Other address traversal/scroll modes
                // are left unsupported rather than interpreted as ST7789.
                if self.colmod&7!=6||self.madctl&0x20!=0 { return false; }
                self.pixel[self.pixeln]=value;self.pixeln+=1;
                if self.pixeln==3 {
                    self.pixeln=0;if self.remaining==0{return true;}
                    let mut x=usize::from(self.x);let mut y=usize::from(self.y);
                    if x<320&&y<480 {
                        if self.madctl&0x40!=0{x=319-x;}
                        if self.madctl&0x80!=0{y=479-y;}
                        self.gram[y*320+x]=(u32::from(self.pixel[0]>>2)<<12)|(u32::from(self.pixel[1]>>2)<<6)|u32::from(self.pixel[2]>>2);
                        self.pixels_written+=1;
                    }
                    self.remaining-=1;
                    if self.x>=self.x1{self.x=self.x0;self.y=self.y.wrapping_add(1);}else{self.x=self.x.wrapping_add(1);}
                }
            }
            0x3c=>return false, // RAMWRC traversal not part of this bounded subset.
            _=>{},
        }
        true
    }
    pub fn rgb565(value:u32)->u16 { (((value>>13)&31)<<11|((value>>6)&63)<<5|((value>>1)&31))as u16 }
}

#[cfg(test)] mod tests {
    use super::*;
    fn command(p:&mut Ili9488,c:u8,data:&[u8]){assert!(p.byte(false,c));for &b in data{assert!(p.byte(true,b));}}
    #[test]fn rgb666_uses_three_bytes_preserves_six_bit_components_and_ignores_window_overrun(){
        let mut p=Ili9488::new();command(&mut p,0x2a,&[0,1,0,2]);command(&mut p,0x2b,&[0,3,0,3]);command(&mut p,0x2c,&[]);
        assert!(p.byte(true,0x84));assert!(p.byte(true,0x48));assert_eq!(p.pixels_written,0);
        assert!(p.byte(true,0x0c));assert_eq!(p.gram[3*320+1],(33<<12)|(18<<6)|3);
        assert_eq!(Ili9488::rgb565(p.gram[3*320+1]),(16<<11)|(18<<5)|1);
        for b in [0xfc,0,0,0,0xfc,0] {assert!(p.byte(true,b));}
        assert_eq!(p.pixels_written,2);assert_eq!(p.gram[3*320+1],(33<<12)|(18<<6)|3,"extra pixel is ignored, not wrapped");
        command(&mut p,0x3a,&[0x55]);command(&mut p,0x2c,&[]);assert!(!p.byte(true,0xf8),"SPI565 is not silently interpreted as666");
        command(&mut p,0x3a,&[0x66]);command(&mut p,0xb0,&[0,0xe8]);command(&mut p,0x36,&[0x48]);
        command(&mut p,0x2a,&[0,0,0,0]);command(&mut p,0x2b,&[0,0,0,0]);command(&mut p,0x2c,&[0xfc,0,0]);assert_eq!(p.gram[319],63<<12);
        command(&mut p,0x36,&[0x20]);command(&mut p,0x2c,&[]);assert!(!p.byte(true,0));
        p.reset();assert_eq!(p.colmod,6);assert!(p.gram.iter().all(|&v|v==0));
    }
}

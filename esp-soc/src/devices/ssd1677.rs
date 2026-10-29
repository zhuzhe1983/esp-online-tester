//! SSD1677 digital RAM/refresh subset for the 800x480 PaperMono glass.
//! Sources: Solomon Systech SSD1677 rev1.0 (Nov2018), commands pp24–40;
//! M5GFX 02107b828acb3eb782fccd747863638d64c876d7 Panel_SSD1677.{cpp,hpp}.
//! Gray endpoints follow that driver's absolute/differential LUT classes. Other
//! single-polarity waveforms have a bounded black/white preview; their pulse-dose
//! gray levels, temperature, ghosting and physical refresh duration are not simulated.
use crate::board::{BoardEdge, VirtualCycle};

const RAM_WIDTH: usize = 960;
const RAM_HEIGHT: usize = 680;
const RAM_BYTES: usize = RAM_WIDTH * RAM_HEIGHT / 8;

pub struct Ssd1677 {
    pub frame: Vec<u16>,
    pub frames: u64,
    pub version: u64,
    pub ram_bytes_written: u64,
    width: usize, height: usize, busy_pin: u8, cycles_per_ms: u64,
    planes: [Vec<u8>; 2], command: u8, params: [u8; 105], count: usize,
    entry: u8, x_start: u16, x_end: u16, y_start: u16, y_end: u16, x: u16, y: u16,
    control1: u8, control2: u8, lut: [u8; 105], lut_written: bool,
    mode2_face_odd: bool, asleep: bool, clock: bool, analog: bool,
    cycle: VirtualCycle, busy_until: Option<VirtualCycle>, pending_refresh: bool, edges: Vec<BoardEdge>,
}
impl Ssd1677 {
    pub fn new(width: usize, height: usize, busy_pin: u8, cycles_per_ms: u64) -> Self {
        let width = width.clamp(1, RAM_WIDTH); let height = height.clamp(1, RAM_HEIGHT);
        Self {frame:vec![0xffff;width*height],frames:0,version:0,ram_bytes_written:0,width,height,busy_pin,cycles_per_ms,
            planes:[vec![0xff;RAM_BYTES],vec![0xff;RAM_BYTES]],command:0,params:[0;105],count:0,
            entry:3,x_start:0,x_end:959,y_start:0,y_end:679,x:0,y:0,control1:0,control2:0xff,lut:[0;105],lut_written:false,
            mode2_face_odd:false,asleep:false,clock:false,analog:false,cycle:0,busy_until:None,pending_refresh:false,edges:Vec::new()}
    }
    /// Reset controller state, retaining the bistable optical image and RAM. RAM
    /// power-loss behavior is unspecified here; firmware must initialize its RAM.
    pub fn reset(&mut self) {
        self.command=0; self.count=0; self.entry=3; self.x_start=0; self.x_end=959; self.y_start=0; self.y_end=679; self.x=0; self.y=0;
        self.control1=0; self.control2=0xff; self.lut_written=false; self.mode2_face_odd=false;
        self.asleep=false; self.clock=false; self.analog=false; self.pending_refresh=false;
        if self.busy_until.take().is_some() { self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:false}); }
    }
    pub fn busy(&self) -> bool { self.busy_until.is_some() }
    pub fn next_deadline(&self) -> Option<VirtualCycle> { self.busy_until }
    pub fn take_edges(&mut self) -> Vec<BoardEdge> { std::mem::take(&mut self.edges) }
    fn busy_for(&mut self, ms: u64) {
        if self.busy_until.is_none() { self.edges.push(BoardEdge{cycle:self.cycle,pin:self.busy_pin,level:true}); }
        self.busy_until=Some(self.cycle.saturating_add(self.cycles_per_ms.saturating_mul(ms).max(1)));
    }
    pub fn advance_to(&mut self, cycle: VirtualCycle) {
        self.cycle=cycle;
        if let Some(until)=self.busy_until.filter(|until|*until<=cycle) {
            self.busy_until=None;
            if self.pending_refresh { self.publish(); self.pending_refresh=false; }
            self.edges.push(BoardEdge{cycle:until,pin:self.busy_pin,level:false});
        }
    }
    pub fn write(&mut self, data: bool, bytes: &[u8]) {
        for &byte in bytes {
            if !data { self.command(byte); } else if !self.asleep { self.data(byte); }
        }
    }
    fn command(&mut self, command: u8) {
        if self.asleep { return; }
        self.command=command; self.count=0;
        match command {
            0x12 => { self.reset(); self.busy_for(1); }
            0x20 => {
                if self.control2 & 0x80 != 0 { self.clock=true; }
                if self.control2 & 0x40 != 0 { self.analog=true; }
                self.pending_refresh=self.control2 & 0x04 != 0 && self.clock && self.analog;
                // Bounded digital busy interval, not measured EPD waveform timing.
                self.busy_for(if self.pending_refresh {20} else {1});
                if self.control2 & 0x02 != 0 { self.analog=false; }
                if self.control2 & 0x01 != 0 { self.clock=false; }
            }
            _ => {}
        }
    }
    fn word(&self, at: usize) -> u16 { u16::from_le_bytes([self.params[at],self.params[at+1]]) & 0x03ff }
    fn data(&mut self, byte: u8) {
        if self.command==0x24 || self.command==0x26 { self.write_ram(usize::from(self.command==0x26),byte); return; }
        if self.count < self.params.len() { self.params[self.count]=byte; }
        self.count=self.count.saturating_add(1);
        match (self.command,self.count) {
            (0x10,1) => { self.asleep=byte & 1 != 0; }
            (0x11,1) => self.entry=byte & 7,
            (0x21,1) => self.control1=byte,
            (0x22,1) => self.control2=byte,
            (0x32,105) => { self.lut=self.params; self.lut_written=true; }
            (0x44,4) => { self.x_start=self.word(0); self.x_end=self.word(2); }
            (0x45,4) => { self.y_start=self.word(0); self.y_end=self.word(2); }
            (0x4e,2) => self.x=self.word(0),
            (0x4f,2) => self.y=self.word(0),
            // SSD1677 rev1.0 names 46h RED / 47h BW; the M5 driver labels them
            // oppositely, but initializes both to F7h (all ones) before rendering.
            (0x46 | 0x47,1) => {
                let bank=usize::from(self.command==0x46);
                let sx=if byte & 7 == 7 {RAM_WIDTH} else {8 << (byte & 7)};
                let sy=if (byte>>4) & 7 == 7 {RAM_HEIGHT} else {8 << ((byte>>4)&7)};
                for y in 0..RAM_HEIGHT { for x in 0..RAM_WIDTH/8 {
                    self.planes[bank][y*(RAM_WIDTH/8)+x]=if ((x*8/sx+y/sy)&1==0)==(byte & 0x80 != 0) {0xff} else {0};
                }}
                self.busy_for(1);
            }
            _ => {}
        }
    }
    fn write_ram(&mut self, bank: usize, byte: u8) {
        let x=usize::from(self.x); let y=usize::from(self.y);
        if x < RAM_WIDTH && y < RAM_HEIGHT { self.planes[bank][y*(RAM_WIDTH/8)+x/8]=byte; self.ram_bytes_written+=1; }
        let step_x=|x:u16| if self.entry & 1 != 0 {x.wrapping_add(8)} else {x.wrapping_sub(8)};
        let step_y=|y:u16| if self.entry & 2 != 0 {y.wrapping_add(1)} else {y.wrapping_sub(1)};
        let end_x=if self.entry & 1 != 0 {self.x/8>=self.x_end/8} else {self.x/8<=self.x_end/8};
        let end_y=if self.entry & 2 != 0 {self.y>=self.y_end} else {self.y<=self.y_end};
        if self.entry & 4 == 0 {
            if end_x {self.x=self.x_start;self.y=if end_y {self.y_start} else {step_y(self.y)};} else {self.x=step_x(self.x);}
        } else if end_y {self.y=self.y_start;self.x=if end_x {self.x_start} else {step_x(self.x)};} else {self.y=step_y(self.y);}
    }
    fn lut_drives(&self, row: usize) -> u8 {
        // SSD1677 rev1.0 section 6.6, Tables 6-6/6-7: each group has four
        // MSB-first VS phases; TP=0 skips a phase and RP=0 still runs once.
        // Only collect active voltage kinds: even RP=255 needs no expansion.
        let mut drives=0;
        for group in 0..10 { for phase in 0..4 {
            if self.lut[50+group*5+phase]!=0 {drives|=1 << ((self.lut[row*10+group]>>(6-phase*2))&3);}
        }}
        drives & !1 // VSS is field-free, including between driven phases.
    }
    fn publish(&mut self) {
        let drives=std::array::from_fn::<_,4,_>(|row|self.lut_drives(row));
        // The endpoint subset assumes a field-free VCOM row, as in the cited
        // M5GFX and EDC LUTs. Active VCOM needs an analog voltage model.
        let fixed_vcom=self.lut_drives(4)==0;
        let mode2=self.control2 & 8 != 0;
        // Differential LUTs have field-free hold groups 0 and 3. Decode the
        // supplied waveform rows, rather than identifying a firmware/file hash.
        // CloudZao d12f481f epd_lut_partial.h additionally drives VSH2 in
        // LUT0 to maintain black. SSD1677 Table 6-6 calls code 11 VSH2, not
        // field-free. Recognize this bounded digital transition class; do not
        // infer arbitrary optical endpoints from unmodelled voltage registers.
        let ink4_partial=self.lut[0]==0 && self.lut[1]==0xcc && self.lut[2..10].iter().all(|b|*b==0)
            && self.lut[10]==0x80 && self.lut[11..20].iter().all(|b|*b==0)
            && self.lut[20]==0x40 && self.lut[21..40].iter().all(|b|*b==0);
        let differential=self.lut_written && (self.lut[..10].iter().all(|b|*b==0) || ink4_partial) && self.lut[30..40].iter().all(|b|*b==0);
        // SSD1677 rev1.0 Table 6-5: a monochrome waveform repeats LUT0 as
        // LUT2 and LUT1 as LUT3. CloudZao INK4 d12f481f fast-update LUT uses
        // this arrangement; treating it as M5GFX's four-gray mapping inverts it.
        // Require distinct black/white rows, so an unknown all-equal LUT is not
        // silently promoted to a known two-endpoint waveform.
        let monochrome=self.lut_written && self.lut[..10]==self.lut[20..30] && self.lut[10..20]==self.lut[30..40] && self.lut[..10]!=self.lut[10..20];
        // RAM windows constrain writes, not refresh. In particular, EDC
        // Book's segmented updates leave group 0 outside the written band:
        // its all-VSS row must retain the optical image there. A disabled
        // phase is also a hold, regardless of its stored voltage code.
        let levels=std::array::from_fn::<_,4,_>(|group| {
            if self.lut_written && fixed_vcom && drives[group]==0 {None}
            // M5GFX's VS definitions: VSH1 darkens, VSL lightens. This is
            // only an endpoint preview for unipolar rows, not calibrated
            // grayscale. Mixed-polarity/VSH2 gray waveforms keep the
            // existing four-level mapping; never infer gray from the last
            // nonzero voltage (the official fast LUT ends gray rows in VSH2).
            else if self.lut_written && fixed_vcom && drives[group]==2 {Some(0)}
            else if self.lut_written && fixed_vcom && drives[group]==4 {Some(255)}
            else if differential {match group {1=>Some(255),2=>Some(0),_=>None}}
            else if monochrome {Some(if group & 1 != 0 {255}else{0})}
            else if self.lut_written {Some([255,170,85,0][group])}
            else {Some(if group & 1 != 0 {255}else{0})}
        });
        for gate in 0..self.height { for source in 0..self.width {
            let index=gate*(RAM_WIDTH/8)+source/8; let mask=0x80 >> (source&7);
            let mut bw=u8::from(self.planes[0][index]&mask!=0);let mut red=u8::from(self.planes[1][index]&mask!=0);
            if self.control1 & 0x04 != 0 {bw=0;} else if self.control1 & 0x08 != 0 {bw^=1;}
            if self.control1 & 0x40 != 0 {red=0;} else if self.control1 & 0x80 != 0 {red^=1;}
            if !mode2 && self.mode2_face_odd {std::mem::swap(&mut bw,&mut red);}
            let group=usize::from((red<<1)|bw);
            let pixel=&mut self.frame[(self.height-1-gate)*self.width+source];
            if let Some(gray)=levels[group] {let g=gray as u16;*pixel=((g>>3)<<11)|((g>>2)<<5)|(g>>3);}
        }}
        self.mode2_face_odd=mode2 && !self.mode2_face_odd;
        self.frames+=1;self.version+=1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn send(panel:&mut Ssd1677, command:u8,data:&[u8]) {panel.write(false,&[command]);panel.write(true,data);}
    fn window(panel:&mut Ssd1677) {send(panel,0x11,&[1]);send(panel,0x44,&[0,0,15,0]);send(panel,0x45,&[1,0,0,0]);send(panel,0x4e,&[0,0]);send(panel,0x4f,&[1,0]);}
    #[test]
    fn ram_is_not_optical_until_activation_and_busy_completion() {
        let mut panel=Ssd1677::new(16,2,18,1000);window(&mut panel);send(&mut panel,0x24,&[0x80,0,0xff,0xff]);
        assert!(panel.frame.iter().all(|p|*p==0xffff));send(&mut panel,0x22,&[0xc4]);send(&mut panel,0x20,&[]);
        assert!(panel.busy());panel.advance_to(19999);assert_eq!(panel.frames,0);panel.advance_to(20000);
        assert_eq!(panel.frames,1);assert_eq!(panel.frame[0],0xffff);assert_eq!(panel.frame[1],0);assert_eq!(panel.frame[16],0xffff);
        assert_eq!(panel.take_edges(),[BoardEdge{cycle:0,pin:18,level:true},BoardEdge{cycle:20000,pin:18,level:false}]);
        send(&mut panel,0x22,&[3]);send(&mut panel,0x20,&[]);panel.advance_to(21000);assert_eq!(panel.frames,1,"power-only activation does not fabricate a frame");
        send(&mut panel,0x12,&[]);assert_eq!(panel.frame[1],0,"controller reset retains optical image");
    }
    #[test]
    fn m5gfx_absolute_gray_and_differential_rows_use_real_ram_bits() {
        let mut panel=Ssd1677::new(8,1,18,1);send(&mut panel,0x44,&[0,0,7,0]);send(&mut panel,0x45,&[0,0,0,0]);
        send(&mut panel,0x24,&[0x50]);send(&mut panel,0x26,&[0x30]);
        // Actual M5GFX 02107b8 lut_fast: the middle-gray rows end in VSH2,
        // which must not collapse both of them to a black endpoint.
        let mut lut=[0;105];
        lut[..40].copy_from_slice(&[
            0x55,0x55,0x55,0x55,0x55,0x5a,0xaa,0xaa,0,0,
            0xaa,0x95,0x55,0x55,0x55,0x5a,0x82,0xac,0,0,
            0xaa,0xa5,0x55,0x55,0x55,0x5a,0xac,0,0,0,
            0xaa,0xaa,0xaa,0xaa,0x55,0x55,0x55,0x50,0,0]);
        for group in 0..8 {lut[50+group*5..54+group*5].fill(if group<4 {1}else{2});}
        lut[100..].fill(0x8f);send(&mut panel,0x32,&lut);
        send(&mut panel,0x22,&[0xc7]);send(&mut panel,0x20,&[]);panel.advance_to(20);
        assert_eq!(&panel.frame[..4],&[0xffff,0xad55,0x52aa,0]);
        let mut lut=[0;105];lut[10]=0x6a;lut[11]=0xa0;lut[20]=0x95;lut[21]=0x50;
        lut[50..54].fill(2);lut[55..57].fill(1);lut[100..].fill(0x8f);send(&mut panel,0x32,&lut);
        send(&mut panel,0x22,&[0xcc]);send(&mut panel,0x20,&[]);panel.advance_to(40);
        assert_eq!(&panel.frame[..4],&[0xffff,0xffff,0,0]);
    }
    #[test]
    fn malformed_addresses_and_long_commands_do_not_panic() {
        let mut panel=Ssd1677::new(800,480,18,1);for command in 0..=255 {send(&mut panel,command,&[0xff;256]);}
        panel.reset();send(&mut panel,0x4e,&[0xff,0xff]);send(&mut panel,0x4f,&[0xff,0xff]);send(&mut panel,0x24,&[0;4096]);
        assert_eq!(panel.frame.len(),800*480);
    }
    #[test]
    fn monochrome_lut_uses_bw_bit_independent_of_previous_red_plane() {
        let mut panel=Ssd1677::new(8,1,9,1);send(&mut panel,0x44,&[0,0,7,0]);send(&mut panel,0x45,&[0,0,0,0]);
        send(&mut panel,0x24,&[0x50]);send(&mut panel,0x26,&[0x30]);
        let mut lut=[0;105];lut[0]=0x90;lut[10]=0x60;lut[20]=0x90;lut[30]=0x60;lut[50]=20;lut[51]=20;
        send(&mut panel,0x32,&lut);send(&mut panel,0x22,&[0xc7]);send(&mut panel,0x20,&[]);panel.advance_to(20);
        assert_eq!(&panel.frame[..4],&[0,0xffff,0,0xffff]);
        // Transition from prior 0101 to 1010 using the shipped partial class.
        let mut partial=[0;105];partial[1]=0xcc;partial[10]=0x80;partial[20]=0x40;partial[50]=1;partial[55]=1;partial[56]=1;
        send(&mut panel,0x24,&[0xa0]);send(&mut panel,0x26,&[0x50]);send(&mut panel,0x32,&partial);
        send(&mut panel,0x22,&[0xcf]);send(&mut panel,0x20,&[]);panel.advance_to(40);
        assert_eq!(&panel.frame[..4],&[0xffff,0,0xffff,0]);
    }
    #[test]
    fn field_free_and_zero_duration_phases_retain_the_optical_image() {
        let mut panel=Ssd1677::new(8,1,18,1);send(&mut panel,0x44,&[0,0,7,0]);send(&mut panel,0x45,&[0,0,0,0]);
        send(&mut panel,0x24,&[0x50]);send(&mut panel,0x26,&[0x30]);
        panel.frame[..4].copy_from_slice(&[0x52aa,0,0xffff,0xad55]);
        let before=panel.frame.clone();let mut lut=[0;105];lut[..40].fill(0xff);
        send(&mut panel,0x32,&lut);send(&mut panel,0x22,&[0xc4]);send(&mut panel,0x20,&[]);panel.advance_to(20);
        assert_eq!(panel.frame,before,"nonzero VS codes with TP=0 do not drive the glass");
        lut[..40].fill(0);lut[50..].fill(0xff);
        send(&mut panel,0x32,&lut);send(&mut panel,0x20,&[]);panel.advance_to(40);
        assert_eq!(panel.frame,before,"VSS remains a hold even with maximum TP and RP");
    }
    #[test]
    fn unipolar_endpoints_use_only_enabled_phases_not_ram_group_or_last_vs_code() {
        let mut panel=Ssd1677::new(8,1,18,1);send(&mut panel,0x44,&[0,0,7,0]);send(&mut panel,0x45,&[0,0,0,0]);
        send(&mut panel,0x24,&[0x50]);send(&mut panel,0x26,&[0x30]);panel.frame.fill(0xad55);
        let mut lut=[0;105];lut[0]=0x4b;lut[10]=0x87;lut[20]=0x03;lut[30]=0x80;
        lut[50]=1;lut[54]=255;send(&mut panel,0x32,&lut);send(&mut panel,0x22,&[0xc4]);send(&mut panel,0x20,&[]);
        panel.advance_to(19);assert!(panel.frame.iter().all(|p|*p==0xad55));panel.advance_to(20);
        assert_eq!(&panel.frame[..4],&[0,0xffff,0xad55,0xffff]);
        // Enabling phase D changes group 2 from hold to VSL. This cannot be
        // replaced with an all-zero-row test that ignores TP timing bytes.
        lut[20]=0x02;lut[53]=1;send(&mut panel,0x32,&lut);send(&mut panel,0x20,&[]);panel.advance_to(40);
        assert_eq!(panel.frame[2],0xffff);
    }
    #[test]
    fn edc_book_segmented_lut_preserves_other_bands_and_drives_group_three_white() {
        let mut panel=Ssd1677::new(16,4,18,1);panel.frame.fill(0xad55);
        // EDC Book 3.1.2, 105-byte 32h payload captured during a page turn.
        // Rows: hold, one VSH1 frame, five VSH1 frames, four VSL frames.
        // The uncalibrated short pulses are a black/white preview here.
        let mut lut=[0;105];lut[11]=0x40;lut[20]=0x55;lut[21]=0x40;lut[30]=0x2a;lut[31]=0x80;
        lut[50..54].fill(1);lut[55..57].fill(1);lut[100..].fill(0x8f);send(&mut panel,0x32,&lut);
        send(&mut panel,0x11,&[1]);send(&mut panel,0x44,&[0,0,15,0]);send(&mut panel,0x22,&[0xc4]);
        for band in 0..4 {
            // Clear both RAM planes to the hold group. Previous optical bands
            // must survive even though their backing RAM changes back to zero.
            send(&mut panel,0x45,&[3,0,0,0]);send(&mut panel,0x4e,&[0,0]);send(&mut panel,0x4f,&[3,0]);
            send(&mut panel,0x24,&[0;8]);send(&mut panel,0x26,&[0;8]);
            let gate=3-band;send(&mut panel,0x45,&[gate,0,gate,0]);send(&mut panel,0x4f,&[gate,0]);
            let bw=0x80>>band;send(&mut panel,0x24,&[bw,bw]);send(&mut panel,0x26,&[0xff,0xff]);
            send(&mut panel,0x20,&[]);panel.advance_to(u64::from(band+1)*20);
            for row in 0..4 {for x in 0..16 {
                let expected=if row>usize::from(band) {0xad55}else if x%8==row {0xffff}else{0};
                assert_eq!(panel.frame[row*16+x],expected,"band {band}, row {row}, x {x}");
            }}
        }
        assert_eq!(panel.frames,4);
    }
}

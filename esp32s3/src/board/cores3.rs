//! K128, K128-Lite and K149 use Sch_M5_CoreS3_v1.0; K128-SE has its own V1.0
//! schematic and omits BMI270. K149's H2 RCP and the DIN base are not modeled.
use std::sync::{Arc,Mutex};
use esp_periph::{i2c::I2cDevice,sdmmc::{SdCardConfig,SdStore,card::spi::SpiSdCard}};
use esp_soc::{board::{BoardModel,BoardEdge,VirtualCycle,SpiPins},devices::bm8563::{Bm8563,Bm8563State}};
use super::stackchan_host::CoreS3Host;

const SD_CS:u64=1<<4;
const LCD_CS:u64=1<<3;

#[derive(Clone,Copy)]
pub(super) enum Variant { Standard, Se, Lite, ThreadBr }

pub(super) struct CoreS3 {
    variant:Variant,
    host:CoreS3Host,
    rtc:Arc<Mutex<Bm8563State>>,
    sd:Option<Box<SpiSdCard>>,
    sd_power_cycles:u64,
}

impl CoreS3 {
    pub(super) fn new(variant:Variant)->Self {
        Self{variant,host:CoreS3Host::new(!matches!(variant,Variant::Se)),
            rtc:Arc::new(Mutex::new(Bm8563State::new(crate::periph::CPU_HZ))),sd:None,sd_power_cycles:0}
    }
}

impl BoardModel for CoreS3 {
    fn name(&self)->&'static str {match self.variant {
        Variant::Standard=>"cores3",Variant::Se=>"cores3-se",Variant::Lite=>"cores3-lite",Variant::ThreadBr=>"cores3-thread-br",
    }}
    fn gpio_changes(&mut self,changes:&[(u8,bool)]) {self.host.gpio_changes(changes);}
    fn gpio_output_at(&mut self,cycle:VirtualCycle,changes:&[(u8,bool)],enabled:u64,output:u64) {
        if output&SD_CS!=0 || enabled&SD_CS==0 {
            if let Some(card)=&mut self.sd {card.deselect();}
        }
        self.rtc.lock().expect("CoreS3 RTC mutex poisoned").advance_to(cycle);
        self.host.gpio_output_at(cycle,changes,enabled,output);
    }
    fn gpio_events(&self)->u64 {self.host.gpio_events()}
    fn uses_spi_pins(&self)->bool {true}
    fn spi_transfer_pins(&mut self,host:u8,pins:SpiPins,tx:&[u8],rx_len:usize)->Vec<u8> {
        // The panel and TF socket share CLK36 and MOSI37. Their separate
        // selects are G3/G4. TF returns over G35, which is LCD D/C.
        if pins.sclk&(1<<36)!=0 && pins.mosi&(1<<37)!=0 {
            if let Some(card)=&mut self.sd {
                let (powered,power_cycles)=self.host.sd_power();
                if self.sd_power_cycles!=power_cycles {
                    self.sd_power_cycles=power_cycles;
                    card.power_cycle();
                }
                if powered && (pins.cs&SD_CS!=0 || pins.cs&LCD_CS==0) {
                    let reply=card.transfer(pins.cs&SD_CS!=0,tx,rx_len);
                    if pins.cs&SD_CS!=0 && pins.miso==Some(35) {return reply;}
                }
            }
        }
        self.host.spi_transfer_pins(host,pins,tx,rx_len)
    }
    fn attach_spi_sd_card(&mut self,image:Box<dyn SdStore>,config:SdCardConfig)->Result<String,String> {
        let card=SpiSdCard::new(image,config)?;
        let description=card.describe();
        self.sd=Some(Box::new(card));
        // TF_SW connects to AW9523B P0_4 and pulls low on insertion.
        self.host.sd_inserted();
        Ok(description)
    }
    fn i2c_devices(&mut self)->Vec<(u8,u8,Box<dyn I2cDevice>)> {
        self.host.i2c_devices(vec![(0x51,Box::new(Bm8563::new(self.rtc.clone())))])
    }
    fn advance_to(&mut self,cycle:VirtualCycle) {
        self.rtc.lock().expect("CoreS3 RTC mutex poisoned").advance_to(cycle);
        self.host.advance_to(cycle);
    }
    fn take_edges(&mut self)->Vec<BoardEdge> {self.host.take_edges()}
    fn input_levels(&self)->Vec<(u8,bool)> {self.host.input_levels()}
    fn touch_at(&mut self,cycle:VirtualCycle,x:u16,y:u16,down:bool) {self.host.touch_at(cycle,x,y,down);}
    fn touch(&mut self,x:u16,y:u16,down:bool) {self.host.touch(x,y,down);}
    fn display(&self)->Option<(u32,u32,Vec<u16>,u64)> {self.host.display()}
    fn display_version(&self)->u64 {self.host.display_version()}
    fn display_frames(&self)->u64 {self.host.display_frames()}
    fn gram(&self)->Option<(Vec<u16>,usize,usize)> {self.host.gram()}
    fn set_imu_sample(&mut self,accel_g:[f64;3],gyro_dps:[f64;3])->bool {self.host.set_imu_sample(accel_g,gyro_dps)}
    fn report(&self)->String {format!("[{}] ILI9342C frames={} pixels={} FT6336U=single-contact RTC=calendar-only SPI-SD={}",
        self.name(),self.display_frames(),self.host.pixels_written(),self.sd.as_ref().map_or_else(||"absent".into(),|sd|sd.report()))}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(device:&mut dyn I2cDevice,reg:u8,values:&[u8]) {
        assert!(device.start(false));assert!(device.write(reg));
        for &value in values {assert!(device.write(value));}
        device.stop();
    }
    fn read(device:&mut dyn I2cDevice,reg:u8,len:usize)->Vec<u8> {
        write(device,reg,&[]);assert!(device.start(true));
        let out=(0..len).map(|_|device.read()).collect();device.stop();out
    }

    #[test]
    fn exact_variants_preserve_core_host_and_se_has_no_imu() {
        for (id,name,imu) in [("m5stack-k128","cores3",true),("m5stack-k128-se","cores3-se",false),
            ("m5stack-k128-lite","cores3-lite",true),("m5stack-k149","cores3-thread-br",true)] {
            let mut board=super::super::make_board(id).unwrap();
            assert_eq!(board.name(),name);
            assert!(board.motion_snapshot().is_none(),"standalone product must not expose StackChan base");
            assert!(!board.uses_uart_pins(),"no RCP or robot peer is connected");
            let devices=board.i2c_devices();
            assert!(devices.iter().all(|(_,_,d)|d.pins()==Some((12,11))));
            assert_eq!(devices.iter().filter(|(_,addr,_)|*addr==0x69).count(),if imu {2} else {0});
            assert_eq!(board.set_imu_sample([0.,0.,1.],[0.;3]),imu);
            assert_eq!(devices.iter().filter(|(_,addr,_)|*addr==0x51).count(),2);
        }
    }

    #[test]
    fn card_detect_and_spi_require_inserted_card_aldo4_and_exact_route() {
        let mut board=CoreS3::new(Variant::Standard);
        let mut devices=board.i2c_devices();
        let aw=devices.iter_mut().find(|(bus,addr,_)|*bus==0 && *addr==0x58).unwrap().2.as_mut();
        write(aw,4,&[0x10]);assert_eq!(read(aw,0,1)[0]&0x10,0x10);
        board.attach_spi_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
        assert_eq!(read(aw,0,1)[0]&0x10,0);
        let pins=SpiPins{sclk:1<<36,mosi:1<<37,miso:Some(35),cs:SD_CS};
        let init=|b:&mut CoreS3,pins|{
            b.spi_transfer_pins(2,SpiPins{cs:0,..pins},&[255;10],0);
            b.spi_transfer_pins(2,pins,&[0x40,0,0,0,0,0x95],0);
            b.spi_transfer_pins(2,pins,&[],2)
        };
        let axp=devices.iter_mut().find(|(bus,addr,_)|*bus==0 && *addr==0x34).unwrap().2.as_mut();
        assert_eq!(init(&mut board,pins),[255,255]);
        write(axp,0x95,&[28]);write(axp,0x90,&[8]);
        assert_eq!(init(&mut board,SpiPins{mosi:1<<38,..pins}),[255,255]);
        assert_eq!(init(&mut board,SpiPins{miso:Some(34),..pins}),[255,255]);
        assert_eq!(init(&mut board,pins),[255,1]);
        write(axp,0x90,&[0]);write(axp,0x90,&[8]);
        board.spi_transfer_pins(2,pins,&[0x40,0,0,0,0,0x95],0);
        assert_eq!(board.spi_transfer_pins(2,pins,&[],2),[255,255]);
        assert_eq!(init(&mut board,pins),[255,1]);
    }
}

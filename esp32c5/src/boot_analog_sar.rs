//! Bounded C5 SAR REGI2C target configuration, IDF5.5.3/5.5.4
//! soc/regi2c_saradc.h + hal/adc_ll.h. Zero bytes are explicit virtual
//! initial configuration, not measured analog POR or conversion samples.
#[derive(Clone,Debug,PartialEq,Eq)]
pub(super) struct Sar{pub enabled:bool,bytes:[u8;6],written:u8}
impl Sar{
    pub fn new()->Self{Self{enabled:false,bytes:[0;6],written:0}}
    pub fn enable(&mut self){self.enabled=true;}
    pub fn disable(&mut self){self.enabled=false;}
    pub fn reset_analog(&mut self){let enabled=self.enabled;*self=Self::new();self.enabled=enabled;}
    fn index(reg:u8)->Option<usize>{[0,1,3,4,7,8].iter().position(|r|*r==reg)}
    pub fn transfer(&mut self,reg:u8,data:Option<u8>)->Option<u8>{
        if !self.enabled{return None;}let n=Self::index(reg)?;
        if let Some(data)=data{
            let mask=match reg{0|3=>0xff,1|4=>0x0f,7=>0x43,8=>0x05,_=>return None};
            if data&!mask!=0{return None;}self.bytes[n]=data;self.written|=1<<n;
        }Some(self.bytes[n])
    }
    pub fn codes(&self)->Option<[u16;2]>{
        (self.enabled&&self.written&15==15).then_some([
            u16::from(self.bytes[0])|u16::from(self.bytes[1])<<8,
            u16::from(self.bytes[2])|u16::from(self.bytes[3])<<8])
    }
    pub fn ready(&self)->bool{self.enabled&&self.written==0x3f&&self.bytes[4]==0x40&&self.bytes[5]==5}
    pub fn report(&self)->String{format!("sar_regi2c=virtual_zero_initial written={:#04x} initial_codes={:?}",self.written,self.codes())}
}

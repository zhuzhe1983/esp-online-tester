/* Original SRAM diagnostic, NOT manufacturer firmware and no ROM hooks.
 * Exact C5 transport registers, Waveshare LCD2.73 schematic/BSP pins.
 * Nominal XTAL48 -> SPI24MHz/I2C400kHz/CH32 PWM. All8MiB virtual Quad PSRAM
 * tested by guest, then the framebuffer is read by GDMA from guest-mapped PSRAM.
 * SD block LBA32 is saved, tested, restored and re-read. No filesystem mount.
 * No flash boot, physical entropy, RF, audio or battery claim. */
#include <stdint.h>
#define R(a) (*(volatile uint32_t *)(a))
#define SPI 0x60081000u
#define MSPI 0x60003000u
#define DMA 0x60080000u
#define GPIO 0x60091000u
#define PCR 0x60096000u
#define I2C 0x60004000u
#define RAM 0x42000000u
static volatile uint32_t descriptors[2][3] __attribute__((aligned(4)));
static uint8_t saved_block[512];
static void puts0(const char *p){while(*p)R(0x60000000u)=(uint8_t)*p++;}
static void fail(const char *p){puts0(p);for(;;)__asm__ volatile("wfi");}
static void pause_cycles(unsigned n){unsigned start,now;__asm__ volatile("rdcycle %0":"=r"(start));do{__asm__ volatile("rdcycle %0":"=r"(now));}while(now-start<n);}
static void pin(unsigned n,unsigned signal){R(0x60090000u+4*n)=0x1800;R(GPIO+0xad4+4*n)=signal;R(GPIO+0x38)=1u<<n;}
static void input(unsigned n){R(0x60090000u+4*n)=0x1b00;R(GPIO+0xad4+4*n)=256;R(GPIO+0x3c)=1u<<n;}
static void level(unsigned n,unsigned high){R(GPIO+(high?8:12))=1u<<n;}
static uint32_t memory_cmd(unsigned op,int quad,int address,unsigned addr,int write,unsigned value,unsigned read_bytes,unsigned dummy){
    R(MSPI+8)=quad?(1u<<8)|(1u<<13)|(1u<<24):0;R(MSPI+4)=addr;
    R(MSPI+0x18)=(1u<<31)|(address?1u<<30:0)|(write?1u<<27:0)|(read_bytes?1u<<28:0)|(dummy?1u<<29:0)|(quad?1u<<15:0);
    R(MSPI+0x1c)=(23u<<26)|(dummy?dummy-1:0);R(MSPI+0x20)=(7u<<28)|op;R(MSPI+0x24)=31;R(MSPI+0x28)=read_bytes?read_bytes*8-1:0;
    R(MSPI+0x58)=value;R(MSPI)=1u<<18;while(R(MSPI)&(1u<<18)){}return R(MSPI+0x58);
}
static void psram(void){
    static const unsigned pads[]={15,17,18,20,21,22};for(unsigned n=0;n<6;n++)R(0x60090000u+4*pads[n])=0x800;
    R(PCR+0x18)=5;R(PCR+0x1c)=0x400;R(MSPI+0x34)=1;R(MSPI+0x14)=0x30103;pause_cycles(36000);
    memory_cmd(0xf5,1,0,0,0,0,0,0);memory_cmd(0x66,0,0,0,0,0,0,0);memory_cmd(0x99,0,0,0,0,0,0,0);pause_cycles(24);
    if((memory_cmd(0x9f,0,1,0,0,0,3,0)&0xffffff)!=0x425d0d)fail("C5 VIRTUAL PSRAM ID FAIL\n");
    memory_cmd(0x35,0,0,0,0,0,0,0);R(0x60002040u)=(1u<<20)|(23u<<14)|(5u<<6)|0x34;
    R(0x60002048u)=(7u<<28)|0xeb;R(0x6000204cu)=(7u<<28)|0x38;R(0x60002050u)=1u<<31;R(0x6000203cu)=0;R(0x60002384u)=4;
    for(unsigned page=0;page<128;page++){R(0x60002380u)=page;R(0x6000237cu)=0x600|page;}
    volatile uint32_t *words=(volatile uint32_t *)RAM;
    for(unsigned n=0;n<2097152;n++)words[n]=n^0x5a6b7c8d;
    for(unsigned n=0;n<2097152;n++)if(words[n]!=(n^0x5a6b7c8d))fail("C5 PSRAM8 READBACK FAIL\n");
    puts0("C5 PSRAM8 8388608B READBACK PASS\n");
}
static void i2c_finish(void){R(I2C+4)=(1u<<4)|(1u<<5);while((R(I2C+0x20)&((1u<<7)|(1u<<10)))==0){}if(R(I2C+0x20)&(1u<<10))fail("C5 I2C NACK\n");}
static void i2c_reset_fifo(void){R(I2C+0x24)=0x7ffff;R(I2C+0x18)=0x408b|(3u<<12);}
static void i2c_write(unsigned addr,unsigned reg,unsigned value){
    i2c_reset_fifo();R(I2C+0x1c)=addr<<1;R(I2C+0x1c)=reg;R(I2C+0x1c)=value;
    R(I2C+0x58)=6u<<11;R(I2C+0x5c)=(1u<<11)|(1u<<8)|3;R(I2C+0x60)=2u<<11;i2c_finish();
}
static void i2c_read_n(unsigned addr,const uint8_t *reg,unsigned reg_count,uint8_t *out,unsigned count){
    i2c_reset_fifo();R(I2C+0x1c)=addr<<1;for(unsigned n=0;n<reg_count;n++)R(I2C+0x1c)=reg[n];R(I2C+0x1c)=(addr<<1)|1;
    R(I2C+0x58)=6u<<11;R(I2C+0x5c)=(1u<<11)|(1u<<8)|(reg_count+1);R(I2C+0x60)=6u<<11;R(I2C+0x64)=(1u<<11)|(1u<<8)|1;
    unsigned cmd=0x68;if(count>1){R(I2C+cmd)=(3u<<11)|(count-1);cmd+=4;}R(I2C+cmd)=(3u<<11)|(1u<<10)|1;R(I2C+cmd+4)=2u<<11;i2c_finish();
    for(unsigned n=0;n<count;n++)out[n]=R(I2C+0x1c);
}
static void i2c_read(unsigned addr,unsigned reg,uint8_t *out,unsigned count){uint8_t cmd=reg;i2c_read_n(addr,&cmd,1,out,count);}
static void exio_write16(unsigned reg,unsigned value){
    i2c_reset_fifo();R(I2C+0x1c)=0x24<<1;R(I2C+0x1c)=reg;R(I2C+0x1c)=value&255;R(I2C+0x1c)=value>>8;
    R(I2C+0x58)=6u<<11;R(I2C+0x5c)=(1u<<11)|(1u<<8)|4;R(I2C+0x60)=2u<<11;i2c_finish();
}
static void i2c_init(void){
    pin(27,47);pin(26,46);R(0x60090000u+4*27)=0x1a00;R(0x60090000u+4*26)=0x1a00;R(GPIO+0x2d4+4*47)=0x11b;R(GPIO+0x2d4+4*46)=0x11a;
    R(PCR+0x20)=3;R(PCR+0x20)=1;R(PCR+0x24)=1u<<22;R(I2C)=59;R(I2C+0x38)=60;
    R(I2C+0x40)=0;R(I2C+0x44)=0;R(I2C+0x48)=0;R(I2C+0x4c)=0;
    i2c_write(0x24,5,0);exio_write16(2,1);exio_write16(3,0);pause_cycles(240000);exio_write16(3,1);
    uint8_t levels[2];i2c_read(0x24,4,levels,2);if((levels[0]&1)!=1||levels[1]!=0)fail("C5 V006 RESET INPUT FAIL\n");
    uint8_t value;i2c_read(0x6b,0,&value,1);if(value!=5)fail("C5 QMI WHOAMI FAIL\n");puts0("C5 I2C V006-24 QMI6B PASS\n");
}
static void finish(void){while(R(SPI)&(1u<<24)){};}
static void cpu_write(const uint8_t *p,unsigned n){
    R(SPI+0x30)=0;R(SPI+0x10)=1u<<27;R(SPI+0x1c)=n*8-1;
    for(unsigned i=0;i<n;i+=4){uint32_t w=0;for(unsigned j=0;j<4&&i+j<n;j++)w|=(uint32_t)p[i+j]<<(8*j);R(SPI+0x98+i)=w;}
    R(SPI)=1u<<24;finish();
}
static void lcd(unsigned command,const uint8_t *args,unsigned n){uint8_t c=command;level(4,0);cpu_write(&c,1);if(n){level(4,1);cpu_write(args,n);}}
static uint8_t sd_byte(uint8_t value) {
    R(SPI+0x30)=0;R(SPI+0x10)=(1u<<27)|(1u<<28)|1;R(SPI+0x1c)=7;R(SPI+0x98)=value;R(SPI)=1u<<24;finish();return R(SPI+0x98);
}
static uint8_t command(unsigned c,uint32_t arg) {
    uint8_t p[6]={0x40|c,arg>>24,arg>>16,arg>>8,arg,1};
    if(c==0)p[5]=0x95;if(c==8)p[5]=0x87;
    for(unsigned i=0;i<6;i++)sd_byte(p[i]);
    for(unsigned i=0;i<16;i++){uint8_t r=sd_byte(255);if((r&0x80)==0)return r;}return 255;
}
static uint8_t block_pattern(unsigned i) { return (uint8_t)((i*73u)^0xa5u^(i>>3)); }
static void read_block(unsigned mode) {
    if(command(17,32)!=0)fail("C5 SD CMD17 FAIL\n");uint8_t token=255;for(unsigned i=0;i<20&&token==255;i++)token=sd_byte(255);if(token!=0xfe)fail("C5 SD TOKEN FAIL\n");
    for(unsigned i=0;i<512;i++){uint8_t value=sd_byte(255);if(mode==0)saved_block[i]=value;else if(value!=(mode==1?block_pattern(i):saved_block[i]))fail("C5 SD READBACK FAIL\n");}sd_byte(255);sd_byte(255);
}
static void write_block(unsigned restore) {
    if(command(24,32)!=0)fail("C5 SD CMD24 FAIL\n");sd_byte(255);sd_byte(0xfe);for(unsigned i=0;i<512;i++)sd_byte(restore?saved_block[i]:block_pattern(i));sd_byte(255);sd_byte(255);
    if((sd_byte(255)&31)!=5)fail("C5 SD WRITE FAIL\n");while(sd_byte(255)==0){}
}
static void sd_probe(void) {
    R(SPI+0x20)=0x3f;level(9,1);for(unsigned i=0;i<10;i++)sd_byte(255);level(9,0);
    if(command(0,0)!=1)fail("C5 SD CMD0 FAIL\n");puts0("C5 SD CMD0=01\n");
    if(command(8,0x1aa)!=1)fail("C5 SD CMD8 FAIL\n");uint32_t r7=0;for(unsigned i=0;i<4;i++)r7=(r7<<8)|sd_byte(255);if(r7!=0x1aa)fail("C5 SD R7 FAIL\n");
    unsigned ready=0;for(unsigned i=0;i<20;i++){if(command(55,0)>1)break;if(command(41,1u<<30)==0){ready=1;break;}}if(!ready)fail("C5 SD ACMD41 FAIL\n");
    read_block(0);write_block(0);read_block(1);puts0("C5 SD LBA32 512B READBACK PASS\n");
    write_block(1);read_block(2);level(9,1);puts0("C5 SD ORIGINAL BLOCK RESTORED\n");
}
static uint16_t color(unsigned x,unsigned y,unsigned keys){
    uint16_t p=(uint16_t)((((x/8)%31+1)<<11)|(((y/5)%63+1)<<5)|(((x+y)/11)%31+1));
    if((keys&1)&&x>=16&&x<304&&y>=32&&y<288)p^=0xffff;
    if((keys&2)&&x>=8&&x<96&&y>=8&&y<312)p^=0xffff;
    if((keys&4)&&x>=224&&x<312&&y>=8&&y<312)p^=0xffff;return p;
}
static void draw(unsigned keys){
    volatile uint8_t *pixels=(volatile uint8_t *)RAM;
    for(unsigned y=0;y<320;y++)for(unsigned x=0;x<320;x++){
        uint16_t p=color(x,y,keys);unsigned i=(y*320+x)*3;
        pixels[i]=((p&0xf800)>>8)|((p&0x8000)>>13);pixels[i+1]=(p&0x7e0)>>3;pixels[i+2]=((p&31)<<3)|((p&16)>>2);
    }
    uint8_t cols[]={0,0,1,63},rows[]={0,0,1,63};lcd(0x2a,cols,4);lcd(0x2b,rows,4);lcd(0x2c,0,0);level(4,1);
    for(unsigned row=0;row<320;row+=8){
        unsigned count=(row+8<=320?8:320-row)*960;unsigned first=count<4092?count:4092;unsigned second=count-first;
        descriptors[0][0]=(1u<<31)|(second?0:1u<<30)|(first<<12)|first;descriptors[0][1]=RAM+row*960;descriptors[0][2]=second?(uint32_t)&descriptors[1]:0;
        descriptors[1][0]=(3u<<30)|(second<<12)|second;descriptors[1][1]=RAM+row*960+first;descriptors[1][2]=0;
        R(DMA+0xd0)=12;R(DMA+0xd4)=1u<<12;R(DMA+0x100)=1;R(DMA+0x3b8)=(uint32_t)descriptors;R(DMA+0xe0)=2;
        R(SPI+0x30)=1u<<28;R(SPI+0x10)=1u<<27;R(SPI+0x1c)=count*8-1;R(SPI)=1u<<24;finish();
        if(R(DMA+0x30)&4)fail("C5 GDMA DESCRIPTOR FAIL\n");
        if((descriptors[0][0]|(second?descriptors[1][0]:0))&(1u<<31))fail("C5 GDMA OWNER FAIL\n");R(DMA+0x3c)=0x3f;
    }
}
int main(void){
    psram();i2c_init();pin(4,256);pin(6,56);pin(7,58);pin(8,61);pin(9,256);level(9,1);input(28);input(2);input(3);
    R(0x60090014u)=0x1a00;R(GPIO+0xad4+4*5)=256;R(GPIO+0x3c)=1u<<5;R(GPIO+0x2d4+4*57)=0x105;
    R(PCR+0xc0)=1;R(PCR+0xc4)=1;R(PCR+0xc8)=1u<<22;R(SPI+0xc)=1u<<12;R(SPI+8)=0;R(SPI+0x20)=0x3e;
    R(DMA+0x3c4)=0x40800000;R(DMA+0x3c8)=0x44000000;
    sd_probe();R(SPI+0x20)=0x3e;
    uint8_t mode=0x66,madctl=0x48;lcd(0x01,0,0);lcd(0x11,0,0);lcd(0x3a,&mode,1);lcd(0x36,&madctl,1);lcd(0x21,0,0);lcd(0x29,0,0);
    draw(0);i2c_write(0x24,5,255);puts0("C5 LCD273 READY\n");unsigned keys=0;
    for(;;){
        unsigned input=R(GPIO+0x64);unsigned next=((input&(1u<<28))==0)|(((input&(1u<<2))==0)<<1)|(((input&(1u<<3))==0)<<2);
        if(next!=keys){unsigned changed=next^keys;keys=next;draw(keys);
            if(changed&1)puts0((keys&1)?"C5 BOOT DOWN\n":"C5 BOOT UP\n");
            if(changed&2)puts0((keys&2)?"C5 PREV DOWN\n":"C5 PREV UP\n");
            if(changed&4)puts0((keys&4)?"C5 NEXT DOWN\n":"C5 NEXT UP\n");
        }
    }
}

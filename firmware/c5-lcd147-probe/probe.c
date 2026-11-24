/* Original SRAM diagnostic, NOT Waveshare factory firmware. No ROM hooks.
 * C5 v5.5.4 register map, Waveshare LCD147 BSP1.0.0 schematic/BSP pins.
 * Explicit XTAL48 -> SPI24MHz; LEDC5kHz. No PLL, RF, USB or physical entropy.
 * Saves, tests, then restores SD block LBA32; no filesystem mount claim. */
#include <stdint.h>
#define REG(a) (*(volatile uint32_t *)(a))
#define SPI 0x60081000u
#define DMA 0x60080000u
#define GPIO 0x60091000u
#define PCR 0x60096000u
static uint8_t pixels[172*16*2] __attribute__((aligned(4)));
static volatile uint32_t descriptors[2][3] __attribute__((aligned(4)));
static uint8_t saved_block[512];
static void puts0(const char *p) { while (*p) REG(0x60000000u)=(uint8_t)*p++; }
static void fail(const char *p) { puts0(p); for (;;) __asm__ volatile("wfi"); }
static void pin(unsigned n,unsigned signal) { REG(0x60090000u+4*n)=0x1800;REG(GPIO+0xad4+4*n)=signal;REG(GPIO+0x38)=1u<<n; }
static void level(unsigned n,unsigned high) { REG(GPIO+(high?8:12))=1u<<n; }
static void finish(void) { while (REG(SPI)&(1u<<24)) {} }
static void cpu_write(const uint8_t *p,unsigned n) {
    REG(SPI+0x30)=0;REG(SPI+0x10)=1u<<27;REG(SPI+0x1c)=n*8-1;
    for(unsigned i=0;i<n;i+=4) { uint32_t w=0;for(unsigned j=0;j<4&&i+j<n;j++)w|=(uint32_t)p[i+j]<<(8*j);REG(SPI+0x98+i)=w; }
    REG(SPI)=1u<<24;finish();
}
static void lcd(unsigned command,const uint8_t *args,unsigned n) {
    uint8_t c=command;level(24,0);cpu_write(&c,1);if(n){level(24,1);cpu_write(args,n);}
}
static uint8_t sd_byte(uint8_t value) {
    REG(SPI+0x30)=0;REG(SPI+0x10)=(1u<<27)|(1u<<28)|1;REG(SPI+0x1c)=7;REG(SPI+0x98)=value;REG(SPI)=1u<<24;finish();return REG(SPI+0x98);
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
    REG(SPI+0x20)=0x3f;level(4,1);for(unsigned i=0;i<10;i++)sd_byte(255);level(4,0);
    if(command(0,0)!=1)fail("C5 SD CMD0 FAIL\n");puts0("C5 SD CMD0=01\n");
    if(command(8,0x1aa)!=1)fail("C5 SD CMD8 FAIL\n");uint32_t r7=0;for(unsigned i=0;i<4;i++)r7=(r7<<8)|sd_byte(255);if(r7!=0x1aa)fail("C5 SD R7 FAIL\n");
    unsigned ready=0;for(unsigned i=0;i<20;i++){if(command(55,0)>1)break;if(command(41,1u<<30)==0){ready=1;break;}}if(!ready)fail("C5 SD ACMD41 FAIL\n");
    read_block(0);write_block(0);read_block(1);puts0("C5 SD LBA32 512B READBACK PASS\n");
    write_block(1);read_block(2);level(4,1);puts0("C5 SD ORIGINAL BLOCK RESTORED\n");
}
static uint16_t color(unsigned x,unsigned y,unsigned pressed) {
    uint16_t p=(uint16_t)((((x/6)%31+1)<<11)|(((y/5)%63+1)<<5)|(((x+y)/11)%31+1));
    if(pressed&&x>=16&&x<156&&y>=32&&y<288)p^=0xffff;return p;
}
static void draw(unsigned pressed) {
    REG(SPI+0x20)=0x3e;uint8_t cols[]={0,34,0,205},rows[]={0,0,1,63};lcd(0x2a,cols,4);lcd(0x2b,rows,4);lcd(0x2c,0,0);level(24,1);
    for(unsigned block=0;block<20;block++){
        for(unsigned y=0;y<16;y++)for(unsigned x=0;x<172;x++){uint16_t p=color(x,block*16+y,pressed);unsigned i=(y*172+x)*2;pixels[i]=p>>8;pixels[i+1]=p;}
        descriptors[0][0]=(1u<<31)|(4092u<<12)|4092;descriptors[0][1]=(uint32_t)pixels;descriptors[0][2]=(uint32_t)&descriptors[1];
        descriptors[1][0]=(3u<<30)|(1412u<<12)|1412;descriptors[1][1]=(uint32_t)(pixels+4092);descriptors[1][2]=0;
        REG(DMA+0xd0)=12;REG(DMA+0xd4)=1u<<12;REG(DMA+0x100)=1;REG(DMA+0x3b8)=(uint32_t)descriptors;REG(DMA+0xe0)=2;
        REG(SPI+0x30)=1u<<28;REG(SPI+0x10)=1u<<27;REG(SPI+0x1c)=sizeof(pixels)*8-1;REG(SPI)=1u<<24;finish();
        if(REG(DMA+0x30)&4)fail("C5 GDMA DESCRIPTOR FAIL\n");
        if((descriptors[0][0]|descriptors[1][0])&(1u<<31))fail("C5 GDMA OWNER FAIL\n");
        REG(DMA+0x3c)=0x3f;
    }
}
int main(void) {
    pin(4,256);level(4,1);pin(24,256);pin(26,256);level(26,0);
    pin(7,56);pin(6,58);pin(23,61);pin(10,0);
    REG(0x60090014u)=0x1a00;REG(GPIO+0xad4+4*5)=256;REG(GPIO+0x3c)=1u<<5;REG(GPIO+0x2d4+4*57)=0x100|5;
    REG(0x60090070u)=0x1b00;REG(GPIO+0xad4+4*28)=256;REG(GPIO+0x3c)=1u<<28;
    REG(PCR+0xc0)=1;REG(PCR+0xc4)=1;REG(PCR+0xc8)=1u<<22;REG(SPI+0xc)=1u<<12;REG(SPI+8)=0;
    REG(DMA+0x3c4)=0x40800000;REG(DMA+0x3c8)=0x44000000;
    REG(PCR+0x48)=1;REG(PCR+0x50)=2;REG(PCR+0x4c)=1u<<22;
    REG(0x600070a8u)=10u|(2400u<<5)|(1u<<26);REG(0x60007008u)=1024u<<4;REG(0x6000700cu)=1u<<31;REG(0x60007000u)=21;
    sd_probe();REG(SPI+0x20)=0x3e;level(26,1);uint8_t mode=0x55,madctl=0;lcd(0x11,0,0);lcd(0x3a,&mode,1);lcd(0x36,&madctl,1);lcd(0x21,0,0);lcd(0x29,0,0);
    draw(0);puts0("C5 LCD147 READY SD PASS\n");unsigned state=0;
    for(;;){unsigned next=(REG(GPIO+0x64)&(1u<<28))==0;if(next!=state){state=next;draw(state);puts0(state?"C5 BOOT DOWN\n":"C5 BOOT UP\n");}}
}

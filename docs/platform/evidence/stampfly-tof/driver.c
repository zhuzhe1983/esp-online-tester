#include "vl53lx_api.h"
#include "vl53lx_api_core.h"
#include <stdio.h>
#include <stdbool.h>
#include <string.h>
#include <stdlib.h>
#include "vl53lx_core.h"
extern void model_distance(void*,double);
extern void *model_new(void);
extern void model_wait(void*,uint64_t);
extern bool model_start(void*,uint8_t,bool);
extern bool model_write(void*,uint8_t);
extern uint8_t model_read(void*);
static void *model; static uint64_t now;
VL53LX_Error VL53LX_WriteMulti(VL53LX_DEV d,uint16_t r,uint8_t*p,uint32_t n){
 if(!model_start(model,d->i2c_slave_address,false))return -13;
 model_write(model,r>>8); model_write(model,r); for(uint32_t i=0;i<n;i++)if(!model_write(model,p[i]))return -13;
 return 0;}
VL53LX_Error VL53LX_ReadMulti(VL53LX_DEV d,uint16_t r,uint8_t*p,uint32_t n){
 if(!model_start(model,d->i2c_slave_address,false))return -13;
 model_write(model,r>>8);model_write(model,r);if(!model_start(model,d->i2c_slave_address,true))return -13;
 for(uint32_t i=0;i<n;i++)p[i]=model_read(model);return 0;}
VL53LX_Error VL53LX_WrByte(VL53LX_DEV d,uint16_t r,uint8_t x){return VL53LX_WriteMulti(d,r,&x,1);}
VL53LX_Error VL53LX_WrWord(VL53LX_DEV d,uint16_t r,uint16_t x){uint8_t b[]={x>>8,x};return VL53LX_WriteMulti(d,r,b,2);}
VL53LX_Error VL53LX_WrDWord(VL53LX_DEV d,uint16_t r,uint32_t x){uint8_t b[]={x>>24,x>>16,x>>8,x};return VL53LX_WriteMulti(d,r,b,4);}
VL53LX_Error VL53LX_RdByte(VL53LX_DEV d,uint16_t r,uint8_t*x){return VL53LX_ReadMulti(d,r,x,1);}
VL53LX_Error VL53LX_RdWord(VL53LX_DEV d,uint16_t r,uint16_t*x){uint8_t b[2];int s=VL53LX_ReadMulti(d,r,b,2);*x=(b[0]<<8)|b[1];return s;}
VL53LX_Error VL53LX_RdDWord(VL53LX_DEV d,uint16_t r,uint32_t*x){uint8_t b[4];int s=VL53LX_ReadMulti(d,r,b,4);*x=((uint32_t)b[0]<<24)|(b[1]<<16)|(b[2]<<8)|b[3];return s;}
VL53LX_Error VL53LX_WaitUs(VL53LX_DEV d,int32_t us){now+=us;model_wait(model,us);return 0;}
VL53LX_Error VL53LX_WaitMs(VL53LX_DEV d,int32_t ms){return VL53LX_WaitUs(d,ms*1000);}
VL53LX_Error VL53LX_GetTickCount(VL53LX_DEV d,uint32_t*x){*x=now/1000;return 0;}
VL53LX_Error VL53LX_GetTimerFrequency(int32_t*x){*x=1000000;return 0;}
VL53LX_Error VL53LX_GetTimerValue(int32_t*x){*x=now;return 0;}
VL53LX_Error VL53LX_WaitValueMaskEx(VL53LX_DEV d,uint32_t timeout,uint16_t r,uint8_t v,uint8_t m,uint32_t delay){for(uint32_t i=0;i<=timeout;i++){uint8_t x;int s=VL53LX_RdByte(d,r,&x);if(s)return s;if((x&m)==v)return 0;VL53LX_WaitMs(d,delay?delay:1);}return -7;}
#define REQUIRE(expr) do { if (!(expr)) { fprintf(stderr, "failed line %d: %s\n", __LINE__, #expr); return 1; } } while (0)
#define OK(expr) REQUIRE((expr) == VL53LX_ERROR_NONE)
extern void model_free(void*);

static int scenario(int mode, int distance, int frames, int change) {
    model = model_new(); now = 0; model_distance(model, distance);
    VL53LX_Dev_t dev = {0}; VL53LX_DEV d = &dev; d->i2c_slave_address = 0x29;
    OK(VL53LX_WaitDeviceBooted(d));
    // The official API takes an 8-bit address and writes the 7-bit register. The caller updates the platform handle.
    OK(VL53LX_SetDeviceAddress(d, 0x54)); REQUIRE(d->Data.LLData.stat_nvm.i2c_slave__device_address == 0x2a);
    uint8_t id; d->i2c_slave_address = 0x29; REQUIRE(VL53LX_RdByte(d, 0x10f, &id) != 0);
    d->i2c_slave_address = 0x2a; OK(VL53LX_RdByte(d, 0x10f, &id)); REQUIRE(id == 0xea);
    OK(VL53LX_DataInit(d)); OK(VL53LX_SetDistanceMode(d, mode));
    OK(VL53LX_SetMeasurementTimingBudgetMicroSeconds(d, 33000));
    // Exact StampFly bottom order: clear/start, delay100ms, then StartMeasurement.
    OK(VL53LX_ClearInterruptAndStartMeasurement(d)); VL53LX_WaitMs(d, 100);
    OK(VL53LX_StartMeasurement(d));
    int low = 10000, high = -10000, valid = 0, empty = 0;
    for (int frame = 0; frame < frames; frame++) {
        if (change && frame == 12) { distance = 1000; model_distance(model, distance); }
        if (change && frame == 24) { distance = -1; model_distance(model, distance); }
        uint8_t ready = 1; VL53LX_WaitMs(d, 1);
        OK(VL53LX_GetMeasurementDataReady(d, &ready)); REQUIRE(!ready);
        VL53LX_WaitMs(d, 35); OK(VL53LX_GetMeasurementDataReady(d, &ready)); REQUIRE(ready);
        VL53LX_MultiRangingData_t result = {0}; OK(VL53LX_GetMultiRangingData(d, &result));
        // API GetMultiRangingData overwrites the internal error; explicitly check it too.
        OK(VL53LX_check_ll_driver_rd_state(d));
        REQUIRE(result.StreamCount == (frame < 256 ? frame : 128 + (frame - 256) % 128));
        if (distance < 0 && (!change || frame >= 34)) { REQUIRE(result.NumberOfObjectsFound == 0); empty++; }
        if (distance >= 0 && frame >= 2 && (!change || frame < 12 || frame >= 22)) {
            REQUIRE(result.NumberOfObjectsFound == 1);
            int range = result.RangeData[0].RangeMilliMeter;
            REQUIRE(abs(range - distance) <= 40);
            REQUIRE(result.RangeData[0].RangeStatus == 0);
            if (range < low) low = range; if (range > high) high = range; valid++;
        }
        OK(VL53LX_ClearInterruptAndStartMeasurement(d));
    }
    OK(VL53LX_StopMeasurement(d)); VL53LX_WaitMs(d, 100);
    uint8_t ready = 1; OK(VL53LX_GetMeasurementDataReady(d, &ready)); REQUIRE(!ready);
    printf("mode=%d input=%d frames=%d valid=%d empty=%d range=%d..%d change=%d\n", mode, distance, frames, valid, empty, low, high, change);
    model_free(model); return 0;
}
int main(void) {
    for (int mode = VL53LX_DISTANCEMODE_MEDIUM; mode <= VL53LX_DISTANCEMODE_LONG; mode++) {
        const int distances[] = {-1, 100, 250, 500, 1000, 1500, 2000, 3000, 4000};
        for (unsigned i = 0; i < sizeof(distances)/sizeof(*distances); i++)
            if (scenario(mode, distances[i], 12, 0)) return 1;
    }
    if (scenario(VL53LX_DISTANCEMODE_MEDIUM, 500, 260, 0)) return 1;
    return scenario(VL53LX_DISTANCEMODE_MEDIUM, 500, 40, 1);
}

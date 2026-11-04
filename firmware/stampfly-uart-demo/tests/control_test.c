// SPDX-License-Identifier: MIT
// Host unit tests exercise parser/controller boundaries; actual PWM is tested in the worker probe.
#include "control.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
static bool command(sf_control *s,sf_sample *sample,const char *input,const char *expected){char line[SF_LINE_CAPACITY];snprintf(line,sizeof line,"%s",input);const char *message;bool ok=sf_command(s,sample,line,&message);assert(!strcmp(message,expected));return ok;}
static void stopped(const sf_control *s){assert(s->mode==SF_STOP);for(int i=0;i<4;i++)assert(s->duty[i]==0);}
int main(void){
    sf_control c={0};sf_sample v={0};
    assert(command(&c,&v,"SF1 MOTOR 0.125 0.25 0.5 0.75","MOTOR"));assert(c.mode==SF_MOTOR);assert(c.duty[0]==.125f&&c.duty[3]==.75f);
    const char *bad[]={"SF1 MOTOR NaN 0 0 0","SF1 MOTOR inf 0 0 0","SF1 MOTOR -0.1 0 0 0","SF1 MOTOR 1e99 0 0 0","SF1 MOTOR 0 0 0 1.01","SF1 MOTOR 0 0 0 0x1p128"};
    for(unsigned i=0;i<sizeof bad/sizeof *bad;i++){sf_control before=c;assert(!command(&c,&v,bad[i],"RANGE"));assert(!memcmp(&c,&before,sizeof c));}
    const char *wrong[]={"SF1 MOTOR 0 0 0","SF1 STOP extra","SF1 INVALID","STOP","SF1 MOTOR 0 0 0 0 extra"};
    for(unsigned i=0;i<sizeof wrong/sizeof *wrong;i++){sf_control before=c;assert(!command(&c,&v,wrong[i],"FORMAT"));assert(!memcmp(&c,&before,sizeof c));}
    assert(command(&c,&v,"SF1 STOP","STOP"));stopped(&c);assert(!command(&c,&v,"SF1 TAKEOFF 0.5","NOT_READY"));
    v.imu_valid=true;assert(command(&c,&v,"SF1 TAKEOFF 0.5","TAKEOFF"));sf_step(&c,&v,.005f);assert(c.mode==SF_AUTO&&c.duty[0]>.5f&&c.duty[0]<.66f);
    assert(command(&c,&v,"SF1 LAND","LAND"));stopped(&c); // An aborted unmeasured launch cannot keep climbing.
    assert(command(&c,&v,"SF1 TAKEOFF 0.5","TAKEOFF"));for(int i=0;i<410;i++)sf_step(&c,&v,.005f);stopped(&c);
    v.tof_valid=true;v.height=.1f;assert(command(&c,&v,"SF1 TAKEOFF 0.5","TAKEOFF"));assert(command(&c,&v,"SF1 TARGET 0.5 5 0 0","TARGET"));sf_step(&c,&v,.005f);assert(c.duty[0]>c.duty[1]&&c.duty[2]>c.duty[3]);
    assert(command(&c,&v,"SF1 TARGET 0.5 0 5 0","TARGET"));sf_step(&c,&v,.005f);assert(c.duty[0]>c.duty[2]&&c.duty[1]>c.duty[3]);
    assert(command(&c,&v,"SF1 TARGET 0.5 0 0 20","TARGET"));sf_step(&c,&v,.005f);assert(c.duty[1]>c.duty[0]&&c.duty[2]>c.duty[3]);
    v.tof_valid=false;sf_step(&c,&v,.005f);stopped(&c);
    v.tof_valid=true;v.height=0;assert(command(&c,&v,"SF1 TAKEOFF 0.5","TAKEOFF"));assert(command(&c,&v,"SF1 LAND","LAND"));for(int i=0;i<100;i++)sf_step(&c,&v,.005f);stopped(&c);
    assert(command(&c,&v,"SF1 TAKEOFF 0.5","TAKEOFF"));v.imu_valid=false;sf_step(&c,&v,.005f);stopped(&c);
    puts("PASS: strict parser/no-mutation errors, motor order/mixer signs, abort/timeout/sensor-loss/landing STOP");
}

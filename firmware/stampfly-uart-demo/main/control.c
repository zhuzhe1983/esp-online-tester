// SPDX-License-Identifier: MIT
#include "control.h"
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
static float clamp(float x,float low,float high){return fmaxf(low,fminf(x,high));}
const char *sf_mode_name(sf_mode mode){return (const char*[]){"STOP","MOTOR","AUTO","LAND"}[mode];}
static void stop(sf_control *s){memset(s,0,sizeof(*s));}
bool sf_command(sf_control *s,const sf_sample *sample,char *line,const char **message){
    char *tokens[8],*save=NULL;int count=0;
    for(char *p=strtok_r(line," \t\r",&save);p;p=strtok_r(NULL," \t\r",&save)){if(count==8){*message="FORMAT";return false;}tokens[count++]=p;}
    *message="FORMAT";if(count<2||strcmp(tokens[0],"SF1"))return false;
    const char *verb=tokens[1];int values=!strcmp(verb,"MOTOR")||!strcmp(verb,"TARGET")?4:!strcmp(verb,"TAKEOFF")?1:!strcmp(verb,"STOP")||!strcmp(verb,"LAND")?0:-1;
    if(values<0||count!=values+2)return false;
    float arg[4]={0};for(int i=0;i<values;i++){char *end;errno=0;arg[i]=strtof(tokens[i+2],&end);if(!*tokens[i+2]||*end||errno==ERANGE||!isfinite(arg[i])){*message="RANGE";return false;}}
    if(!strcmp(verb,"MOTOR")){for(int i=0;i<4;i++)if(arg[i]<0||arg[i]>1){*message="RANGE";return false;}stop(s);s->mode=SF_MOTOR;memcpy(s->duty,arg,sizeof arg);}
    else if(!strcmp(verb,"STOP"))stop(s);
    else if(!strcmp(verb,"LAND")){if(s->mode!=SF_AUTO&&s->mode!=SF_LAND){*message="MODE";return false;}if(!s->height_locked)stop(s);else {s->mode=SF_LAND;s->roll_target=s->pitch_target=s->yaw_target=0;}}
    else {
        if(arg[0]<.10f||arg[0]>1.50f||(values==4&&(fabsf(arg[1])>10||fabsf(arg[2])>10||fabsf(arg[3])>45))){*message="RANGE";return false;}
        if(!strcmp(verb,"TARGET")&&s->mode!=SF_AUTO){*message="MODE";return false;}
        if(!sample->imu_valid||fabsf(sample->roll)>15||fabsf(sample->pitch)>15){*message="NOT_READY";return false;}
        if(!strcmp(verb,"TAKEOFF")){if(s->mode!=SF_STOP){*message="MODE";return false;}stop(s);s->height_setpoint=sample->tof_valid?sample->height:0;s->height_locked=sample->tof_valid;s->mode=SF_AUTO;}
        s->height_target=arg[0];s->roll_target=arg[1];s->pitch_target=arg[2];s->yaw_target=arg[3];
    }
    *message=verb;return true;
}
void sf_step(sf_control *s,const sf_sample *v,float dt){
    if(s->mode==SF_STOP){memset(s->duty,0,sizeof s->duty);return;}
    if(s->mode==SF_MOTOR)return;
    // Fail closed on stale measurements, extreme attitude or a missed control deadline.
    if(!v->imu_valid||(s->height_locked&&!v->tof_valid)||fabsf(v->roll)>50||fabsf(v->pitch)>50||dt<=0||dt>.10f){stop(s);return;}
    s->flight_seconds+=dt;if(v->tof_valid)s->height_locked=true;
    if(!s->height_locked&&s->flight_seconds>2.0f){stop(s);return;}
    float goal=s->mode==SF_LAND?0:s->height_target;
    s->height_setpoint+=clamp(goal-s->height_setpoint,-.20f*dt,.30f*dt);
    float error=s->height_setpoint-v->height;s->height_integral=clamp(s->height_integral+error*dt,-1,1);
    float collective=clamp(.612459f+.10f*error-.10f*v->vertical_speed+.02f*s->height_integral,.25f,.90f);
    if(!s->height_locked)collective=.50f+fminf(.16f,s->flight_seconds*.20f);
    float roll=clamp( .0008f*(s->roll_target-v->roll)-.0002f*v->p,-.15f,.15f);
    float pitch=clamp( .0010f*(s->pitch_target-v->pitch)-.00025f*v->q,-.15f,.15f);
    float yaw=clamp(.0007f*(s->yaw_target-v->r),-.08f,.08f);
    s->duty[0]=clamp(collective+roll+pitch-yaw,0,1);s->duty[1]=clamp(collective-roll+pitch+yaw,0,1);
    s->duty[2]=clamp(collective+roll-pitch+yaw,0,1);s->duty[3]=clamp(collective-roll-pitch-yaw,0,1);
    if(s->mode==SF_LAND&&s->height_setpoint<.005f&&v->height<.025f){s->land_seconds+=dt;if(s->land_seconds>.3f)stop(s);}else s->land_seconds=0;
}

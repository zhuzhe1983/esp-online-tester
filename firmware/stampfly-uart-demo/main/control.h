// SPDX-License-Identifier: MIT
#pragma once
#include <stdbool.h>
#include <stdint.h>
#define SF_VERSION "0.1.0"
#define SF_LINE_CAPACITY 160
// FRD axes; motor order FL/G5, FR/G42, RL/G10, RR/G41.
typedef enum {SF_STOP,SF_MOTOR,SF_AUTO,SF_LAND} sf_mode;
typedef struct {float height,vertical_speed,roll,pitch,p,q,r;bool imu_valid,tof_valid;} sf_sample;
typedef struct {sf_mode mode;float duty[4],height_target,roll_target,pitch_target,yaw_target,height_setpoint,height_integral,land_seconds,flight_seconds;bool height_locked;} sf_control;
const char *sf_mode_name(sf_mode mode);
// Returns ACK verb or ERR reason. Failed commands leave the controller unchanged.
bool sf_command(sf_control *state,const sf_sample *sample,char *line,const char **message);
void sf_step(sf_control *state,const sf_sample *sample,float dt);

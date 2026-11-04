// SPDX-License-Identifier: MIT
#pragma once
#include "control.h"
void sf_sensors_init(void);
void sf_sensors_update(sf_sample *sample,float dt,int64_t now);
void sf_sensors_report(void);

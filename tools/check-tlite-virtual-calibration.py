#!/usr/bin/env python3
"""Check the published virtual MLX90640 vector with the pinned manufacturer math.
This compiles the unmodified manufacturer parameter/conversion struct on the host;
it does not execute an ESP32 image or validate physical radiometry.
"""
import argparse,gzip,hashlib,json,pathlib,subprocess,tempfile
p=argparse.ArgumentParser();p.add_argument('--source',type=pathlib.Path,required=True);p.add_argument('--header',type=pathlib.Path,required=True);a=p.parse_args()
def read(path):
 b=path.read_bytes();return gzip.decompress(b) if path.suffix=='.gz' else b
source=read(a.source);header=read(a.header)
assert hashlib.sha256(source).hexdigest()=='4e83f614bd190c12ca777488675cab8bb52623a20ebeff53bf08d16d8329278a'
assert hashlib.sha256(header).hexdigest()=='4f6e58673b5434b807ec8ec0ef7804e16341d50c05d8f9a8471ce0acaf7de257'
s=source.decode();body=s[s.index('namespace m5 {'):s.index('static MLX90640_params_t MLX90640_params;')]
with tempfile.TemporaryDirectory(prefix='tlite-nominal-') as temporary:
 d=pathlib.Path(temporary);(d/'mlx.hpp').write_bytes(header)
 (d/'check.cpp').write_text('#include "mlx.hpp"\n#include <algorithm>\n#include <cstring>\n#include <cmath>\n#include <cstdio>\n'+body+'}\n'+'int main(){uint16_t e[832]={},f[834]={},out[384]={};e[7]=0x5349;e[8]=0x4d00;e[17]=8000;e[33]=1024;e[48]=1000;e[49]=29127;e[50]=8;e[51]=0x01ff;e[52]=0x1111;e[54]=0x0101;e[55]=0x0101;e[56]=0x2f00;e[57]=100;e[63]=0x1440;for(int i=64;i<832;i++)e[i]=0x10;for(int i=0;i<768;i++)f[i]=8000;f[768]=1000;f[778]=1000;f[800]=1000;f[810]=static_cast<uint16_t>(-8224);f[832]=0x1901;f[833]=0;m5::MLX90640_params_t params;params.setParam(e);float vdd=params.MLX90640_GetVdd(f),ta=params.MLX90640_GetTa(f);if(!std::isfinite(vdd)||!std::isfinite(ta))return 2;params.MLX90640_CalculateTo(f,.95f,ta-8,out);auto baseline=out[0];f[0]=9000;params.MLX90640_CalculateTo(f,.95f,ta-8,out);printf("{\\"nominalVdd\\":%.6f,\\"virtualAmbientC\\":%.6f,\\"baselineRawQ7\\":%u,\\"hotspotRawQ7\\":%u}\\n",vdd,ta,baseline,out[0]);return out[0]>baseline?0:3;}\n')
 subprocess.run(['g++','-std=c++17','-O0',str(d/'check.cpp'),'-o',str(d/'check')],check=True)
 out=subprocess.check_output([str(d/'check')],text=True,timeout=10);result=json.loads(out)
 result.update(sourceSha256='4e83f614bd190c12ca777488675cab8bb52623a20ebeff53bf08d16d8329278a',headerSha256='4f6e58673b5434b807ec8ec0ef7804e16341d50c05d8f9a8471ce0acaf7de257',method='pinned-manufacturer-C++-parameter-extraction-and-conversion',physicalCalibration=False)
 print(json.dumps(result,indent=2))

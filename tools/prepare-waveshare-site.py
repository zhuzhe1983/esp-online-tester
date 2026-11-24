#!/usr/bin/env python3
"""Stage immutable incremental downloads and the static Sites front end."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]

def save(path,value):
    path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--state',required=True,type=Path)
    p.add_argument('--patches',required=True,type=Path)
    p.add_argument('--site',required=True,type=Path)
    p.add_argument('--complete',action='store_true')
    p.add_argument('--skip-parts-through',type=int,default=0,help='Keep full metadata but do not stage an already verified R2 prefix; uploader must verify its canonical gzip and active immutable manifest.')
    a=p.parse_args();state=json.loads(a.state.read_text());assert 0<=a.skip_parts_through<=len(state['batches'])
    dist=a.site/'dist';downloads=dist/'downloads';downloads.mkdir(parents=True,exist_ok=True)
    template=ROOT/'tools/waveshare-batches/site'
    for name in ['index.html','app.js','style.css']:shutil.copyfile(template/name,dist/name)
    shutil.copyfile(template/'apply-waveshare.py',downloads/'apply-waveshare.py')
    manifest={**state,'totalListings':149,'totalSkus':384,'complete':a.complete,
        'baseTree':subprocess.check_output(['git','rev-parse',state['baseCommit']+'^{tree}'],cwd=ROOT,text=True).strip(),
        'updatedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')}
    for b in manifest['batches']:
        compressed=(a.patches/b['gzipFilename']).read_bytes()
        assert hashlib.sha256(compressed).hexdigest()==b['gzipSha256']
        b['parts']=[]
        for at in range(0,len(compressed),8*1024*1024):
            body=compressed[at:at+8*1024*1024]
            name=b['gzipFilename']+'.part'+str(at//(8*1024*1024)+1).zfill(3)
            target=downloads/name
            if target.exists():assert target.read_bytes()==body,'Previously published part must be immutable'
            elif b['number']>a.skip_parts_through:target.write_bytes(body)
            b['parts'].append({'filename':name,'bytes':len(body),'sha256':hashlib.sha256(body).hexdigest()})
        save(downloads/f'waveshare-{b["number"]:03d}.json',b)
    save(downloads/'manifest.json',manifest)
    shutil.copyfile(ROOT/'docs/platform/research/waveshare-20261124/waveshare-complete-inventory.json',downloads/'inventory.json')
    text=f'''Waveshare ESP32 增量适配补丁\n\n基础仓库：https://github.com/zhuzhe1983/esp-online-tester\n基础提交：{state['baseCommit']}\n已发布：{len(state['batches'])} 批，{len(state['listings'])} 款型号家族，{state.get('skuCount',len(state['productIds']))} 个 SKU\n\n每批包含5款官方商品型号家族（最后一批可少于5款），同一型号的包装/容量变体列为独立SKU。\n补丁严格增量：001基于上述提交，002基于001的结果，依此类推。不要将每一批都应用到原始基础提交。\n\n用法：\n1. 将全部已发布补丁下载并解压 ZIP，保留 .diff.gz 文件和 manifest.json。\n2. 在项目目录创建基于基础提交的干净分支：\n   git switch -c waveshare-adaptation {state['baseCommit']}\n3. 验证全链（不修改真实工作区和索引）：\n   python /补丁目录/apply-waveshare.py --check\n4. 应用（先全链验证，再依次 git apply --index）：\n   python /补丁目录/apply-waveshare.py --apply\n5. 自行审查暂存改动并提交；按项目文档运行 tools/wasm-build.sh，生成浏览器使用的 WASM。\n\n也可解压每个 .diff.gz 后按编号手工 git apply --index。manifest.json 包含原始与压缩文件SHA-256、父提交和目标树。\n脚本识别已经应用的批次树，可以在同一个精确树上继续后续批次；它不会重置、stash、提交或推送GitHub。\n如果你在该分支加入其他修改，需自行整合，脚本不会猜测或覆盖它们。\n\n验证边界：\n型号/封装识别、3D模型来源、数字协议仿真、完整原厂固件尝试、浏览器/WebGL和真实硬件分别记录。\ntrial表示允许尝试，不代表模拟通过。参考型固件缺少完整启动路径时保留来源说明。\n没有连接实物进行硬件验收。每批的具体结果与未支持功能见批次JSON以及仓库knowledge/evidence。\n官方未提供统一发布日期；顺序采用带屏优先及商品排序参考，未虚构上市日期。\n\n更新：{manifest['updatedAt']}\n'''
    (downloads/'README.txt').write_text(text)
    print(json.dumps({'site':str(a.site),'batches':len(state['batches']),'listings':len(state['listings']),
        'downloadBytes':sum(p.stat().st_size for p in downloads.iterdir() if p.is_file())},ensure_ascii=False))
if __name__=='__main__':main()

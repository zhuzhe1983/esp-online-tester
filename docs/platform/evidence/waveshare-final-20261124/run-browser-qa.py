#!/usr/bin/env python3
"""Run the repository's existing browser contract against an isolated loopback server."""
import argparse
import datetime as dt
import functools
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import threading
import time

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--repo', type=Path, required=True)
p.add_argument('--selection', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--playwright-module', required=True)
p.add_argument('--chromium', default='/usr/bin/chromium')
a=p.parse_args()
repo, out, selection=a.repo.resolve(),a.output.resolve(),a.selection.resolve()
out.mkdir(parents=True,exist_ok=True)

class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*args):
        pass

server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(QuietHandler,directory=str(repo/'web')))
thread=threading.Thread(target=server.serve_forever,daemon=True)
thread.start()
origin=f'http://127.0.0.1:{server.server_port}/platform/'
command=['node','tools/check-waveshare-platform-browser.mjs',origin,str(out),str(selection)]
env=dict(os.environ,PLAYWRIGHT_MODULE=a.playwright_module,CHROMIUM_PATH=a.chromium)
record={'startedAt':dt.datetime.now(dt.timezone.utc).isoformat(),
        'command':['node','tools/check-waveshare-platform-browser.mjs','$LOOPBACK/platform/','$OUTPUT','$SELECTION'],
        'selectionSha256':hashlib.sha256(selection.read_bytes()).hexdigest(),
        'harnessSha256':hashlib.sha256((repo/'tools/check-waveshare-platform-browser.mjs').read_bytes()).hexdigest(),
        'method':'Fresh headless Chromium contexts, isolated profile, loopback HTTP only, SwiftShader WebGL; no firmware loading.',
        'limitations':['No physical hardware validation.','Checks the final sixteen product profiles, not all previously published models.']}
start=time.monotonic()
try:
    with (out/'browser-command.log').open('w') as log:
        process=subprocess.Popen(command,cwd=repo,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,errors='replace')
        for line in process.stdout:
            line=line.replace(str(repo),'$REPO').replace(str(out),'$OUTPUT').replace(str(selection),'$SELECTION').replace(origin,'$LOOPBACK/platform/')
            log.write(line); log.flush(); print(line,end='',flush=True)
        record['exitCode']=process.wait()
finally:
    server.shutdown(); server.server_close(); thread.join(timeout=5)
record.update(finishedAt=dt.datetime.now(dt.timezone.utc).isoformat(),durationSeconds=round(time.monotonic()-start,3),
              logSha256=hashlib.sha256((out/'browser-command.log').read_bytes()).hexdigest())
(out/'command.json').write_text(json.dumps(record,indent=2)+'\n')
raise SystemExit(record['exitCode'])

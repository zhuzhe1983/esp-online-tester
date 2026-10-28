#!/usr/bin/env python3
"""Serve web/ for local platform development: python3 tools/serve-platform.py.

Keep a large accept queue for the buildless ES module graph, which can open many
connections at once. Disable development caching so catalog/module edits appear
after a normal reload. This server stores no firmware or user device data.
"""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class Handler(SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Cache-Control', 'no-store')
        super().end_headers()


class Server(ThreadingHTTPServer):
    request_queue_size = 128
    daemon_threads = True


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8088)
    parser.add_argument('--bind', default='127.0.0.1')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'web'
    with Server((args.bind, args.port), partial(Handler, directory=str(root))) as server:
        print(f'ESP Workbench: http://{args.bind}:{args.port}/platform/', flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass

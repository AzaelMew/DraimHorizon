#!/usr/bin/env python3
"""Run the actual Rust party clients against the Python relay on loopback."""
import os
from pathlib import Path
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]

def main():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    env = os.environ.copy()
    env['RELAY_ACCESS_TOKEN'] = ''
    env['RELAY_DEEPL_API_KEY'] = ''
    env['DH_RELAY_E2E_URL'] = f'ws://127.0.0.1:{port}/v1/relay'
    log = ROOT / 'target/relay-e2e.log'
    log.parent.mkdir(exist_ok=True)
    with log.open('w') as output:
        server = subprocess.Popen([sys.executable, '-m', 'dhrelay', '--host', '127.0.0.1', '--port', str(port)], cwd=ROOT / 'relay', env=env, stdout=output, stderr=output)
        try:
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            deadline = time.monotonic() + 10
            while True:
                try:
                    with opener.open(f'http://127.0.0.1:{port}/healthz', timeout=1) as response:
                        if response.status == 200:
                            break
                except OSError:
                    if server.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError('local relay did not become ready; see target/relay-e2e.log')
                    time.sleep(.1)
            subprocess.run(['cargo', 'run', '--locked', '--example', 'relay_smoke', '--quiet'], cwd=ROOT, env=env, check=True)
        finally:
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
    return 0

if __name__ == '__main__':
    raise SystemExit(main())

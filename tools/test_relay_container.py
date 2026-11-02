#!/usr/bin/env python3
"""Build the deployment image and check its health as its non-root runtime user."""
from pathlib import Path
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]

def main():
    env = os.environ.copy()
    for key in ('DOCKER_HOST', 'DOCKER_CONTEXT', 'DOCKER_TLS', 'DOCKER_TLS_VERIFY', 'DOCKER_CERT_PATH'):
        env.pop(key, None)
    docker = ['docker', '--host=unix:///var/run/docker.sock']
    build = docker + ['build', '-q', '-t', 'claim-borders-relay:verify']
    if env.get('CODEX_PROXY_CERT'):
        build += ['--secret', 'id=proxy_ca,src=/etc/ssl/certs/ca-certificates.crt']
    subprocess.run(build + [str(ROOT / 'relay')], env=env, check=True, stdout=subprocess.DEVNULL)
    name = 'claim-relay-container-test'
    subprocess.run(docker + ['run', '-d', '--name', name, 'claim-borders-relay:verify'], env=env, check=True, stdout=subprocess.DEVNULL)
    try:
        deadline = time.monotonic() + 10
        while True:
            code = "import urllib.request; assert urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://127.0.0.1:8765/healthz',timeout=1).status==200"
            result = subprocess.run(docker + ['exec', name, 'python', '-c', code], env=env, capture_output=True)
            if result.returncode == 0:
                print('PASS: deployment container health, non-root user and readable source')
                break
            if time.monotonic() > deadline:
                subprocess.run(docker + ['logs', '--tail', '10', name], env=env)
                raise RuntimeError('relay container failed its health check')
            time.sleep(.1)
    finally:
        subprocess.run(docker + ['rm', '-f', name], env=env, stdout=subprocess.DEVNULL, check=False)
        subprocess.run(docker + ['image', 'rm', 'claim-borders-relay:verify'], env=env, stdout=subprocess.DEVNULL, check=False)

if __name__ == '__main__':
    main()

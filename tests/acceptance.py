#!/usr/bin/env python3
"""Execute the frozen fixture over HTTP against a newly launched process."""
import json
import os
from pathlib import Path
import re
import subprocess
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
fixture = json.loads((ROOT / 'acceptance/fixtures/DEMO-02/cases.json').read_text())
subprocess.run(['cargo', 'build', '--locked', '--bin', 'demo02'], cwd=ROOT, check=True)
env = {**os.environ, **fixture['service']['env']}
base = 'http://127.0.0.1:' + env['PORT']
# Refuse to run against an existing server.
try:
    urllib.request.urlopen(base + fixture['service']['ready'], timeout=1)
except (urllib.error.URLError, TimeoutError):
    pass
else:
    raise RuntimeError('port already serves HTTP; stop the existing service first')
process = subprocess.Popen([str(ROOT / 'target/debug/demo02')], env=env)
saved = {}

def expand(value):
    if isinstance(value, dict):
        return {k: expand(v) for k, v in value.items()}
    if isinstance(value, list):
        return [expand(v) for v in value]
    if isinstance(value, str):
        value = re.sub(r'\$\{repeat:(.):(\d+)\}', lambda m: m.group(1) * int(m.group(2)), value)
        return re.sub(r'\$\{(\w+)\}', lambda m: str(saved[m.group(1)]), value)
    return value

def request(method, path, body=None):
    req = urllib.request.Request(base + path, data=body, method=method, headers={'Content-Type': 'application/json'})
    try:
        response = urllib.request.urlopen(req, timeout=3)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        return response.status, response.read()

try:
    for _ in range(100):
        if process.poll() is not None:
            raise RuntimeError('service exited before becoming ready')
        try:
            if request('GET', fixture['service']['ready'])[0] == 200:
                break
        except urllib.error.URLError:
            time.sleep(0.05)
    else:
        raise RuntimeError('service did not become ready')
    for case in fixture['http']:
        case = expand(case)
        body = json.dumps(case['body'], ensure_ascii=False).encode() if 'body' in case else case.get('raw_body', '').encode() if 'raw_body' in case else None
        status, data = request(case['method'], case['path'], body)
        assert status == case['status'], (case['name'], status, data)
        value = json.loads(data) if data else None
        for key, expected in case.get('json', {}).items():
            assert value[key] == expected, (case['name'], key, value)
        for key in case.get('json_has', []):
            assert key in value, (case['name'], key)
        if 'json_array_length' in case:
            assert isinstance(value, list) and len(value) == case['json_array_length'], case['name']
        if 'json_array_ids' in case:
            assert [item['id'] for item in value] == case['json_array_ids'], case['name']
        for name, field in case.get('save', {}).items():
            saved[name] = value[field]
        print('PASS', case['name'])
finally:
    process.terminate()
    process.wait(timeout=5)

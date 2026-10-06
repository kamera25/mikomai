"""Verify helpers through the distributable worker, without an installed runtime."""
import json
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
WORKER = os.environ.get('MIKOMAI_DEVICE_WORKER', str(ROOT / 'target/debug/mikomai-device-worker-macos-arm64'))
CONFIG = 'hostname fixture\ninterface GigabitEthernet0/1\n description test\n'
requests = [
    dict(op='config_validate', config=CONFIG),
    dict(op='config_convert', config=CONFIG, target_vendor='juniper'),
    dict(op='nwdiag_render', schema='nwdiag { network "検証ネットワーク" { a [label="ルータ"]; } }'),
]
worker = subprocess.Popen([WORKER], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
try:
    for index, request in enumerate(requests):
        request.update(version=1, id=str(index), timeout=30)
        worker.stdin.write(json.dumps(request) + '\n')
        worker.stdin.flush()
        result = json.loads(worker.stdout.readline())
        assert result['status'] == 'completed', result
        assert result['version'] == 1 and result['id'] == str(index)
        if request['op'] == 'nwdiag_render':
            svg = result['payload']['svg']
            assert 'data:font/ttf;base64,' in svg and 'ルータ' in svg
        print('PASS bundled worker ' + request['op'])
finally:
    worker.terminate()
    worker.wait(timeout=10)

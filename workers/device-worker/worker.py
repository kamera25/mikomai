#!/usr/bin/env python3
"""Versioned, persistent stdin/stdout worker. Never accept credentials via argv/env."""
import contextlib
import io
import json
import queue
import sys
import threading
from pathlib import Path

VERSION = 1
ROOT = Path(getattr(sys, '_MEIPASS', Path(__file__).resolve().parents[2] / 'mikomai-core' / 'assets'))
sys.path.insert(0, str(ROOT / 'network'))
requests = queue.Queue()
cancelled = set()
output_lock = threading.Lock()


def emit(message):
    with output_lock:
        sys.__stdout__.write(json.dumps(message, ensure_ascii=False) + '\n')
        sys.__stdout__.flush()


def execute(req):
    op = req['op']
    if op == 'dry_run':
        commands = req.get('commands', [])
        results = [{'command': c, 'ok': bool(c.strip()) and '\n' not in c and '\r' not in c and '\x00' not in c} for c in commands]
        return {'results': results, 'success': bool(results) and all(r['ok'] for r in results)}
    if op in ('config_validate', 'config_convert'):
        from config_helper import validate_config, convert_config
        if op == 'config_validate':
            return validate_config(req.get('config', ''))
        return convert_config(req.get('config', ''), req.get('target_vendor', 'juniper'), ROOT / 'templates')
    if op == 'nwdiag_render':
        import tempfile
        import nwdiag_wrapper
        from blockdiag.utils import bootstrap
        from types import SimpleNamespace
        from nwdiag import parser, builder, drawer
        tree = parser.parse_string(req['schema'])
        diagram = builder.ScreenNodeBuilder.build(tree)
        font = ROOT / 'fonts' / 'NotoSansJP-Regular.ttf'
        if not font.is_file():
            raise RuntimeError('Bundled Japanese font is missing')
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / 'network.svg'
            image = drawer.DiagramDraw('SVG', diagram, filename=str(target), fontmap=bootstrap.create_fontmap(SimpleNamespace(fontmap=None,font=str(font))))
            image.draw(); image.save()
            svg = target.read_text()
            if len(svg.encode()) > 4 * 1024 * 1024:
                raise RuntimeError('Diagram exceeds 4 MiB')
            return {'success': True, 'svg': svg}
    if op not in ('show', 'config', 'console'):
        raise ValueError('Unknown worker operation')
    import netmiko_patches
    from netmiko import ConnectHandler
    credentials = req.get('credentials', {})
    transport = req['transport']
    driver = req['device_type']
    if transport == 'telnet' and not driver.endswith('_telnet'):
        driver = driver.removesuffix('_ssh') + '_telnet'
    if transport == 'serial' and not driver.endswith('_serial'):
        driver = driver.removesuffix('_ssh') + '_serial'
    connection = {'device_type': driver, 'username': req.get('username', ''),
                  'password': credentials.get('password', ''), 'secret': credentials.get('enablePassword', ''),
                  'timeout': req['timeout'], 'conn_timeout': req['timeout'], 'auth_timeout': req['timeout']}
    if transport == 'serial':
        connection['serial_settings'] = {'port': req['host'], 'baudrate': req.get('baud_rate', 9600)}
    else:
        connection.update(host=req['host'], port=req.get('port', 23 if transport == 'telnet' else 22))
    # Library prints go to an in-memory sink, never the protocol stdout.
    with contextlib.redirect_stdout(io.StringIO()):
        net = ConnectHandler(**connection)
        try:
            if req['id'] in cancelled:
                raise RuntimeError('Cancelled before sending')
            if op == 'show':
                texts = [net.send_command(c, read_timeout=req['timeout'], cmd_verify=False) for c in req['commands']]
                return {'success': True, 'output': '\n'.join(texts)}
            if connection['secret']:
                net.enable()
            # Emit on the real protocol stream before the first mutating call.
            with contextlib.redirect_stdout(sys.__stdout__):
                emit({'version': VERSION, 'id': req['id'], 'phase': 'sending'})
            if op == 'console':
                text = net.send_command_timing(req['message'], read_timeout=req['timeout'])
            else:
                text = net.send_config_set(req['commands'], read_timeout=req['timeout'], cmd_verify=False)
            return {'success': True, 'output': text}
        finally:
            net.disconnect()


def run():
    while True:
        req = requests.get()
        if req is None:
            return
        response = {'version': VERSION, 'id': req.get('id', ''), 'status': 'failed'}
        try:
            if req.get('version') != VERSION or not req.get('id'):
                raise ValueError('Invalid protocol version/id')
            if not isinstance(req.get('timeout'), (int, float)) or not 0 < req['timeout'] <= 300:
                raise ValueError('Invalid timeout')
            if req['id'] in cancelled:
                response['status'] = 'cancelled'
            else:
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    payload = execute(req)
                response.update(status='completed' if payload.get('success', True) else 'failed', payload=payload)
        except Exception:
            # Exceptions can contain credentials or command contents.
            response['error'] = 'Worker operation failed'
        emit(response)
        cancelled.discard(req.get('id'))
        req.clear()


if __name__ == '__main__':
    thread = threading.Thread(target=run, daemon=True); thread.start()
    for line in sys.stdin:
        try:
            request = json.loads(line)
            if request.get('op') == 'cancel' and request.get('version') == VERSION:
                cancelled.add(request.get('id'))
            else:
                requests.put(request)
        except (ValueError, TypeError):
            emit({'version': VERSION, 'id': '', 'status': 'failed', 'error': 'Invalid JSON request'})
    requests.put(None); thread.join()

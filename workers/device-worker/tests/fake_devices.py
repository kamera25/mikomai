"""Loopback Cisco-style SSH/Telnet and PTY fixtures for the production CLI/worker."""
import json
import os
import pty
import tty
import socket
import select
import subprocess
import tempfile
import threading
import uuid
from pathlib import Path
import paramiko

ROOT = Path(__file__).resolve().parents[3]
CLI = ROOT / 'target/debug/mikomai-cli'
commands_seen = []
device_state = {"hostname":'fixture'}


def dialog(read, write, login=False):
    if login:
        write(b'Username: ')
        read(4096)
        write(b'Password: ')
        read(4096)
    write(b'\r\nfixture#')
    buffer = b''
    config = False
    while True:
        try:
            data = read(4096)
        except (OSError, EOFError):
            break
        if not data:
            break
        buffer += data.replace(b'\r', b'\n')
        while b'\n' in buffer:
            raw, buffer = buffer.split(b'\n', 1)
            command = raw.decode(errors='replace').strip()
            commands_seen.append(command)
            output = ''
            if command == 'configure terminal': config = True
            elif command in ('end', 'exit'): config = False
            elif command.startswith('hostname '):
                import time
                time.sleep(0.15)
                device_state['hostname'] = command.split(' ',1)[1]
            elif command == 'show version': output = 'Cisco IOS Software, CLI fixture\r\n'
            elif command == 'show running-config': output = 'hostname ' + device_state['hostname'] + '\r\n'
            prompt = 'fixture(config)#' if config else 'fixture#'
            try: write((command + '\r\n' + output + prompt).encode())
            except (OSError, EOFError): return


class Server(paramiko.ServerInterface):
    def check_auth_password(self, username, password):
        return paramiko.AUTH_SUCCESSFUL if username == 'fixture' and password == 'fixture-secret' else paramiko.AUTH_FAILED
    def get_allowed_auths(self, username): return 'password'
    def check_channel_request(self, kind, chanid): return paramiko.OPEN_SUCCEEDED
    def check_channel_pty_request(self, *args): return True
    def check_channel_shell_request(self, channel): return True


def listen(ssh):
    sock = socket.socket(); sock.bind(('127.0.0.1', 0)); sock.listen()
    key = paramiko.RSAKey.generate(2048) if ssh else None
    def accept():
        while True:
            try: client, _ = sock.accept()
            except OSError: return
            def serve(client=client):
                if ssh:
                    transport = paramiko.Transport(client); transport.add_server_key(key)
                    try:
                        transport.start_server(server=Server()); channel = transport.accept(10)
                        if channel: dialog(channel.recv, channel.sendall)
                    except (OSError, EOFError, paramiko.SSHException): pass
                    finally: transport.close()
                else:
                    try: dialog(client.recv, client.sendall, login=True)
                    finally: client.close()
            threading.Thread(target=serve, daemon=True).start()
    threading.Thread(target=accept, daemon=True).start()
    return sock, sock.getsockname()[1]


def main():
    ssh, ssh_port = listen(True); telnet, telnet_port = listen(False)
    master, slave = pty.openpty(); tty.setraw(slave); serial_path = os.ttyname(slave)
    serial_stop = threading.Event()
    def serial_read(count):
        while not serial_stop.is_set():
            ready, _, _ = select.select([master], [], [], 0.1)
            if ready:
                return os.read(master, count)
        return b''
    serial_thread = threading.Thread(target=dialog, args=(serial_read, lambda data: os.write(master, data)), daemon=True)
    serial_thread.start()
    with tempfile.TemporaryDirectory(prefix='mikomai-fake-devices-') as directory:
        env = dict(os.environ, MIKOMAI_GRAPH_DB_PATH=directory+'/db', MIKOMAI_DATA_DIR=directory+'/data', MIKOMAI_DEVICE_WORKER=str(ROOT/'target/debug/mikomai-device-worker-macos-arm64'))
        def cli(*args, stdin=None):
            result = subprocess.run([str(CLI), *args], input=stdin, text=True, capture_output=True, env=env, timeout=120)
            assert result.returncode == 0, (args, result.stderr, result.stdout)
            assert 'fixture-secret' not in result.stdout + result.stderr
            return result.stdout.strip()
        # C# owns the canonical DB while independent CLI processes use its broker.
        dotnet=os.environ.get('MIKOMAI_DOTNET','/usr/local/share/dotnet/dotnet' if Path('/usr/local/share/dotnet/dotnet').exists() else 'dotnet')
        env['DYLD_LIBRARY_PATH']=str(ROOT/'target/debug')
        owner=subprocess.Popen([dotnet,str(ROOT/'contracts/csharp/bin/Debug/net9.0/Contracts.dll'),'--serve',str(ROOT/'contracts/fixtures/task-lifecycle.json')],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        assert json.loads(owner.stdout.readline())['version']==1
        ids = [str(uuid.uuid4()) for _ in range(3)]
        devices = [dict(id=ids[i],name=transport+'-fixture',host=serial_path if transport=='Console' else '127.0.0.1',username='fixture',deviceType='cisco_ios',connectionType=transport,port=str(port),hasPassword=True,hasEnablePassword=False) for i,(transport,port) in enumerate([('SSH',ssh_port),('Telnet',telnet_port),('Console',0)])]
        try:
            for id in ids: cli('credentials-stdin',id,stdin=json.dumps({'password':'fixture-secret'}))
            for device in devices: cli('device-register',device['id'],device['name'],device['host'],device['deviceType'],device['connectionType'],device['port'],device['username'])
            for device in devices:
                result = json.loads(cli('device-show',device['id'],'show version'))
                assert 'Cisco IOS Software, CLI fixture' in result['output'], result
                print('PASS CLI → Rust → bundled worker → '+device['connectionType'])
            plan = json.loads(cli('native-query',json.dumps({'op':'operation_prepare','id':ids[0],'proposal':'hostname approved-fixture','rationale':'fixture verification'})))
            # Unapproved execution must fail before any mutating device command.
            failure = subprocess.run([str(CLI),'operation-execute',plan['id'],plan['planHash']],capture_output=True,text=True,env=env,timeout=20)
            assert failure.returncode != 0
            assert 'configure terminal' not in commands_seen
            cli('operation-approve',plan['id'],plan['planHash'])
            result = json.loads(cli('operation-execute',plan['id'],plan['planHash']))
            assert result['verification'] == 'configuration_read_back'
            assert 'hostname approved-fixture' in result['after_config']
            assert '+ hostname approved-fixture' in result['diff']
            replay = subprocess.run([str(CLI),'operation-execute',plan['id'],plan['planHash']],capture_output=True,text=True,env=env,timeout=20)
            assert replay.returncode != 0
            assert commands_seen.count('configure terminal') == 1
            print('PASS immutable approval, single send, pre/post verification, replay rejection')
            plans=[]
            for hostname in ['fixture-one','fixture-two']:
                plan=json.loads(cli('native-query',json.dumps({'op':'operation_prepare','id':ids[0],'proposal':'hostname '+hostname,'rationale':'cross-process fixture'})))
                cli('operation-approve',plan['id'],plan['planHash']);plans.append(plan)
            writers=[subprocess.Popen([str(CLI),'operation-execute',plan['id'],plan['planHash']],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True) for plan in plans]
            for writer in writers:
                output,error=writer.communicate(timeout=120);assert writer.returncode==0,(output,error)
            sequence=[command for command in commands_seen if command=='configure terminal' or command=='end' or command.startswith('hostname ')]
            assert commands_seen.count('configure terminal')==3
            for start in range(0,len(sequence),3):assert sequence[start]=='configure terminal' and sequence[start+1].startswith('hostname ') and sequence[start+2]=='end',sequence
            print('PASS C# store owner + concurrent CLI writers: shared canonical DB and exclusive device writes')
        finally:
            for id in ids: cli('credentials-stdin',id,stdin=json.dumps({'password':'','enablePassword':''}))
            owner.stdin.write('stop\n');owner.stdin.flush();owner.wait(timeout=10)
    ssh.close(); telnet.close(); serial_stop.set(); serial_thread.join(timeout=2)
    assert not serial_thread.is_alive(), "serial fixture did not stop"
    os.close(master); os.close(slave)


if __name__ == '__main__': main()

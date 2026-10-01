import contextlib
import io
import json
import unittest
from unittest.mock import MagicMock, patch

import netmiko_wrapper


class NetworkTransportTests(unittest.TestCase):
    def run_request(self, payload):
        connector = MagicMock()
        connector.return_value.send_command.return_value = "router configuration"
        with patch("sys.argv", ["netmiko_wrapper", "--stdin"]), \
             patch("sys.stdin", io.StringIO(json.dumps(payload) + "\n")), \
             patch.object(netmiko_wrapper, "ConnectHandler", connector), \
             patch.object(netmiko_wrapper.time, "sleep"), \
             contextlib.redirect_stdout(io.StringIO()) as output, \
             contextlib.redirect_stderr(io.StringIO()), \
             self.assertRaises(SystemExit) as result:
            netmiko_wrapper.main()
        return connector, result.exception.code, output.getvalue()

    def test_telnet_and_ssh_forward_custom_ports_and_commands(self):
        for device_type, port in [("cisco_ios_telnet", "23"), ("cisco_ios_telnet", "2323"), ("cisco_ios", "2222")]:
            with self.subTest(device_type=device_type, port=port):
                connector, status, output = self.run_request({
                    "action": "show", "host": "192.0.2.1", "device_type": device_type,
                    "port": port, "username": "admin", "password": "password",
                    "secret": "enable", "commands": ["show running-config"],
                })
                self.assertEqual(status, 0)
                self.assertEqual(connector.call_args.kwargs["port"], int(port))
                self.assertEqual(connector.call_args.kwargs["device_type"], device_type)
                self.assertEqual(connector.call_args.kwargs["secret"], "enable")
                self.assertIn("router configuration", output)
                connector.return_value.send_command.assert_called_once()
                connector.return_value.disconnect.assert_called_once()

    def test_omitted_port_keeps_netmiko_default(self):
        connector, status, _ = self.run_request({
            "action": "show", "host": "192.0.2.1", "device_type": "cisco_ios_telnet",
            "commands": ["show version"],
        })
        self.assertEqual(status, 0)
        self.assertNotIn("port", connector.call_args.kwargs)

    def test_invalid_port_fails_before_connecting(self):
        for port in ["bad", "0", "65536"]:
            connector, status, _ = self.run_request({
                "action": "show", "host": "192.0.2.1", "device_type": "cisco_ios_telnet",
                "port": port, "commands": ["show version"],
            })
            self.assertEqual(status, 1)
            connector.assert_not_called()


if __name__ == "__main__":
    unittest.main()

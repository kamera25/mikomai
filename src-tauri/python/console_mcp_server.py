import sys
import json
import platform
import os

try:
    from mcp.server.fastmcp import FastMCP
except ImportError:
    print("Error: mcp is not installed. Please run 'pip install mcp'", file=sys.stderr)
    sys.exit(1)

try:
    import netmiko_patches
    from netmiko import ConnectHandler, NetmikoAuthenticationException, NetmikoTimeoutException
except ImportError:
    print("Error: netmiko is not installed. Please run 'pip install netmiko'", file=sys.stderr)
    sys.exit(1)

# Instantiate FastMCP server
mcp = FastMCP("ConsoleMCP")

def get_config_path() -> str:
    """
    Returns the path to the device configuration JSON file.
    It checks the CONSOLE_DEVICES_JSON environment variable first,
    then defaults to a local 'console_devices.json' file.
    """
    return os.environ.get("CONSOLE_DEVICES_JSON", "console_devices.json")

def load_device_config(target_name: str) -> dict:
    """
    Loads the device configuration from the JSON file by target name.
    """
    config_path = get_config_path()
    if not os.path.exists(config_path):
        raise FileNotFoundError(f"Config file not found: {config_path}")

    with open(config_path, "r", encoding="utf-8") as f:
        config = json.load(f)

    # Assuming the JSON contains a list of devices under a 'devices' key
    devices = config.get("devices", [])
    if isinstance(config, list):
        devices = config

    for device in devices:
        if device.get("name") == target_name:
            return device

    raise ValueError(f"Device '{target_name}' not found in configuration.")

def get_serial_port(device_config: dict) -> str:
    """
    Determines the appropriate serial port based on the current OS.
    """
    ports = device_config.get("ports", {})
    sys_name = platform.system()

    if sys_name == "Windows":
        port = ports.get("windows")
    elif sys_name in ["Linux", "Darwin"]:
        # Darwin is macOS, which behaves like Unix for serial ports (e.g., /dev/tty.*)
        port = ports.get("unix")
    else:
        # Fallback
        port = ports.get("unix") or ports.get("windows")

    if not port:
        raise ValueError(f"No serial port configured for OS '{sys_name}' on this device.")

    return port

@mcp.tool()
def send_console_command(target_name: str, command: str) -> str:
    """
    Sends a console command to a network device using a direct serial connection via Netmiko.
    """
    try:
        device_config = load_device_config(target_name)
        serial_port = get_serial_port(device_config)

        # Build netmiko device dict
        netmiko_params = {
            "device_type": device_config.get("device_type", "cisco_ios_serial"),
            "username": device_config.get("username", ""),
            "password": device_config.get("password", ""),
            "secret": device_config.get("secret", ""),
            "serial_settings": {
                "port": serial_port
            }
        }

        # Add any additional serial settings from config if present
        if "baudrate" in device_config:
            netmiko_params["serial_settings"]["baudrate"] = device_config["baudrate"]
        if "bytesize" in device_config:
            netmiko_params["serial_settings"]["bytesize"] = device_config["bytesize"]
        if "parity" in device_config:
            netmiko_params["serial_settings"]["parity"] = device_config["parity"]
        if "stopbits" in device_config:
            netmiko_params["serial_settings"]["stopbits"] = device_config["stopbits"]

        # Connect using Netmiko
        net_connect = ConnectHandler(**netmiko_params)

        # Handle enable mode if secret is provided and we want to run privileged commands
        if netmiko_params["secret"]:
            net_connect.enable()

        # Send the command and get the output
        output = net_connect.send_command(command)

        # Disconnect gracefully
        net_connect.disconnect()

        return output

    except NetmikoAuthenticationException as e:
        return f"Authentication Error: {str(e)}"
    except NetmikoTimeoutException as e:
        return f"Connection Timeout Error: {str(e)}"
    except Exception as e:
        return f"Error: {str(e)}"

if __name__ == "__main__":
    # Start the FastMCP stdio server
    mcp.run()

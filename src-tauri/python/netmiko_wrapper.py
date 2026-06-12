import argparse
import sys
import json
try:
    import netmiko_patches
    from netmiko import ConnectHandler
except ImportError:
    print("Error: netmiko is not installed. Please run 'pip install netmiko'")
    sys.exit(1)

def main():
    parser = argparse.ArgumentParser(description="Netmiko Wrapper for Tauri App")
    parser.add_argument("--stdin", action="store_true", help="Read arguments as JSON from stdin")
    parser.add_argument("--action", required=False, choices=["show", "config"])
    parser.add_argument("--host", required=False)
    parser.add_argument("--username", required=False)
    parser.add_argument("--password", required=False, default="")
    parser.add_argument("--secret", required=False, default="")
    parser.add_argument("--device_type", required=False)
    parser.add_argument("--command", required=False)
    parser.add_argument("--commands", required=False, help="JSON list of commands for config")
    parser.add_argument("--console_port", required=False)
    parser.add_argument("--console_baud_rate", required=False)
    
    args = parser.parse_args()
    args.console_port = None
    args.console_baud_rate = None

    if args.stdin or not sys.stdin.isatty():
        try:
            # Read a single line from stdin to avoid blocking for EOF
            input_data = sys.stdin.readline()
            if input_data.strip():
                data = json.loads(input_data)
                args.action = data.get("action")
                args.host = data.get("host")
                args.username = data.get("username")
                args.password = data.get("password", "")
                args.secret = data.get("secret", "")
                args.device_type = data.get("device_type")
                args.command = data.get("command")
                args.console_port = data.get("console_port")
                args.console_baud_rate = data.get("console_baud_rate")
                if "commands" in data:
                    args.commands = json.dumps(data["commands"]) if isinstance(data["commands"], list) else data["commands"]
        except json.JSONDecodeError as e:
            print(f"Error parsing stdin JSON: {str(e)}", file=sys.stderr)
            sys.exit(1)

    required_fields = [args.action, args.device_type]
    if not args.console_port:
        required_fields.append(args.host)

    if not all(required_fields):
        print("Error: action, host (if not using console), and device_type are required.", file=sys.stderr)
        sys.exit(1)

    if args.console_port:
        device_type = args.device_type
        if not device_type.endswith("_serial"):
            device_type = f"{device_type}_serial"
        
        device = {
            "device_type": device_type,
            "username": args.username or "",
            "password": args.password or "",
            "secret": args.secret or "",
            "serial_settings": {
                "port": args.console_port,
                "baudrate": int(args.console_baud_rate) if args.console_baud_rate else 9600
            },
            "session_log": None
        }
    else:
        device = {
            "device_type": args.device_type,
            "host": args.host,
            "username": args.username or "",
            "password": args.password or "",
            "secret": args.secret or "",
            "session_log": None
        }

    try:
        net_connect = ConnectHandler(**device)
        
        if args.action == "show":
            if not args.command:
                print("Error: command is required for 'show' action", file=sys.stderr)
                sys.exit(1)
            output = net_connect.send_command(args.command)
            print(output)
            
        elif args.action == "config":
            if not args.commands:
                print("Error: commands is required for 'config' action", file=sys.stderr)
                sys.exit(1)
            commands_list = json.loads(args.commands) if isinstance(args.commands, str) else args.commands
            output = net_connect.send_config_set(commands_list)
            print(output)
            
        net_connect.disconnect()
        sys.exit(0)
        
    except Exception as e:
        print(f"Netmiko Error: {str(e)}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()

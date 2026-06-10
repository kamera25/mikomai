import argparse
import sys
import json
try:
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
    
    args = parser.parse_args()

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
                if "commands" in data:
                    args.commands = json.dumps(data["commands"]) if isinstance(data["commands"], list) else data["commands"]
        except json.JSONDecodeError as e:
            print(f"Error parsing stdin JSON: {str(e)}", file=sys.stderr)
            sys.exit(1)

    if not all([args.action, args.host, args.username, args.device_type]):
        print("Error: action, host, username, and device_type are required.", file=sys.stderr)
        sys.exit(1)

    device = {
        "device_type": args.device_type,
        "host": args.host,
        "username": args.username,
        "password": args.password,
        "secret": args.secret,
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

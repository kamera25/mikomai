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
    parser.add_argument("--action", required=True, choices=["show", "config"])
    parser.add_argument("--host", required=True)
    parser.add_argument("--username", required=True)
    parser.add_argument("--password", required=False, default="")
    parser.add_argument("--device_type", required=True)
    parser.add_argument("--command", required=False)
    parser.add_argument("--commands", required=False, help="JSON list of commands for config")
    
    args = parser.parse_args()

    device = {
        "device_type": args.device_type,
        "host": args.host,
        "username": args.username,
        "password": args.password,
        "session_log": None
    }

    try:
        net_connect = ConnectHandler(**device)
        
        if args.action == "show":
            if not args.command:
                print("Error: --command is required for 'show' action", file=sys.stderr)
                sys.exit(1)
            output = net_connect.send_command(args.command)
            print(output)
            
        elif args.action == "config":
            if not args.commands:
                print("Error: --commands is required for 'config' action", file=sys.stderr)
                sys.exit(1)
            commands_list = json.loads(args.commands)
            output = net_connect.send_config_set(commands_list)
            print(output)
            
        net_connect.disconnect()
        sys.exit(0)
        
    except Exception as e:
        print(f"Netmiko Error: {str(e)}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()

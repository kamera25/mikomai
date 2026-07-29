import argparse
import sys
import json
import time
import re
try:
    import netmiko_patches
    from netmiko import ConnectHandler
except ImportError:
    print("Error: netmiko is not installed. Please run 'pip install netmiko'")
    sys.exit(1)

def send_command_wait_for_prompt(net_connect, command, read_timeout=120.0):
    try:
        if hasattr(net_connect, "disable_paging"):
            net_connect.disable_paging()
    except Exception:
        pass

    prompt = ""
    try:
        prompt = net_connect.find_prompt().strip()
    except Exception:
        pass

    if hasattr(net_connect, "clear_buffer"):
        net_connect.clear_buffer()
    time.sleep(0.5)

    expect_pattern = rf"{re.escape(prompt)}|[#>$]" if prompt else r"[#>$]"

    try:
        output = net_connect.send_command(
            command,
            expect_string=expect_pattern,
            read_timeout=read_timeout,
            delay_factor=2.0,
            cmd_verify=False
        )
    except Exception:
        try:
            output = net_connect.send_command_timing(
                command,
                read_timeout=read_timeout,
                delay_factor=2.0
            )
        except Exception as e:
            print(f"INFO: Command execution note: {str(e)}", file=sys.stderr, flush=True)
            output = ""

    # Check for Pager prompts (--More--) and fetch remaining output until prompt returns
    more_patterns = [r"--More--", r"-- more --", r"--- more ---", r"Press any key", r"--\s*More\s*--"]
    max_loops = 200
    while max_loops > 0:
        found_more = False
        for pat in more_patterns:
            if re.search(pat, output, re.IGNORECASE):
                found_more = True
                break
        if not found_more:
            break
        
        max_loops -= 1
        more_output = ""
        try:
            more_output = net_connect.send_command_timing(" ", read_timeout=10.0, delay_factor=1.5)
        except Exception:
            break
        output += "\n" + more_output

    return output

def main():
    parser = argparse.ArgumentParser(description="Netmiko Wrapper for Tauri App")
    parser.add_argument("--stdin", action="store_true", help="Read arguments as JSON from stdin")
    parser.add_argument("--action", required=False, choices=["show", "config", "dry_run"])
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
            "global_delay_factor": 2.0,
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
            "global_delay_factor": 2.0,
            "session_log": None
        }

    try:
        host_desc = args.console_port if args.console_port else args.host
        print(f"INFO: Connecting to device {host_desc} ({args.device_type})...", file=sys.stderr, flush=True)
        net_connect = ConnectHandler(**device)
        print("INFO: Connected successfully to device.", file=sys.stderr, flush=True)
        
        if args.action == "show":
            if not args.command:
                print("Error: command is required for 'show' action", file=sys.stderr, flush=True)
                sys.exit(1)

            # Ensure privileged mode (enable) is active before running show command (e.g. show running-config)
            try:
                if hasattr(net_connect, "check_enable_mode") and not net_connect.check_enable_mode():
                    print("INFO: Entering privileged mode (enable)...", file=sys.stderr, flush=True)
                    net_connect.enable()
                    print("INFO: Entered privileged mode successfully.", file=sys.stderr, flush=True)
            except Exception as enable_err:
                print(f"INFO: Privilege mode transition note: {str(enable_err)}", file=sys.stderr, flush=True)

            print(f"INFO: Executing show command: {args.command}", file=sys.stderr, flush=True)
            output = send_command_wait_for_prompt(net_connect, args.command)
            print(output, flush=True)
            
        elif args.action == "config":
            if not args.commands:
                print("Error: commands is required for 'config' action", file=sys.stderr, flush=True)
                sys.exit(1)
            commands_list = json.loads(args.commands) if isinstance(args.commands, str) else args.commands
            print(f"INFO: Sending configuration commands ({len(commands_list)} lines) via Netmiko...", file=sys.stderr, flush=True)
            if hasattr(net_connect, "clear_buffer"):
                net_connect.clear_buffer()
            time.sleep(0.5)
            output = net_connect.send_config_set(commands_list, read_timeout=90.0, delay_factor=2.0)
            print(output, flush=True)
            print("INFO: Configuration deployment completed successfully.", file=sys.stderr, flush=True)

        elif args.action == "dry_run":
            if not args.commands:
                print("Error: commands is required for 'dry_run' action", file=sys.stderr, flush=True)
                sys.exit(1)
            commands_list = json.loads(args.commands) if isinstance(args.commands, str) else args.commands
            print(f"INFO: Entering configure mode for Dry-run validation ({len(commands_list)} lines)...", file=sys.stderr, flush=True)

            try:
                if hasattr(net_connect, "check_enable_mode") and not net_connect.check_enable_mode():
                    print("INFO: Entering privileged mode (enable)...", file=sys.stderr, flush=True)
                    net_connect.enable()
                    print("INFO: Entered privileged mode successfully.", file=sys.stderr, flush=True)
            except Exception as enable_err:
                print(f"INFO: Privilege mode transition note: {str(enable_err)}", file=sys.stderr, flush=True)

            try:
                net_connect.config_mode()
            except Exception as conf_err:
                print(f"INFO: Config mode entry note: {str(conf_err)}", file=sys.stderr, flush=True)

            error_keywords = ["error", "% invalid", "% ambiguous", "% syntax error", "unrecognized", "incomplete command"]
            results = []

            for cmd in commands_list:
                print(f"INFO: Dry-run verifying line: {cmd}", file=sys.stderr, flush=True)
                out = ""
                try:
                    out = net_connect.send_command_timing(
                        f"{cmd}\t",
                        read_timeout=10.0,
                        delay_factor=1.5
                    )
                except Exception as e:
                    out = str(e)

                lower_out = out.lower()
                has_err = any(kw in lower_out for kw in error_keywords)

                if has_err:
                    err_msg = out.strip() or "Command produced error on Tab completion"
                    results.append({"line": cmd, "ok": False, "output": out, "error": err_msg})
                else:
                    results.append({"line": cmd, "ok": True, "output": out, "error": None})

                if hasattr(net_connect, "clear_buffer"):
                    net_connect.clear_buffer()

            try:
                net_connect.exit_config_mode()
            except Exception:
                pass

            has_errors = any(not r["ok"] for r in results)
            print(json.dumps({"success": not has_errors, "results": results}), flush=True)
            print("INFO: Dry-run validation completed.", file=sys.stderr, flush=True)
            
        net_connect.disconnect()
        print("INFO: Disconnected from device.", file=sys.stderr, flush=True)
        sys.exit(0)
        
    except Exception as e:
        print(f"Netmiko Error: {str(e)}", file=sys.stderr, flush=True)
        sys.exit(1)

if __name__ == "__main__":
    main()


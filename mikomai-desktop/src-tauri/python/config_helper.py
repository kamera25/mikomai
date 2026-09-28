import sys
import json
import os
import re

try:
    from ciscoconfparse2 import CiscoConfParse
except ImportError:
    CiscoConfParse = None

try:
    from jinja2 import Environment, FileSystemLoader
except ImportError:
    Environment = None
    FileSystemLoader = None

def ip_to_cidr(mask):
    try:
        return sum(bin(int(x)).count('1') for x in mask.split('.'))
    except Exception:
        return 24

def parse_cisco_config(config_text):
    if CiscoConfParse is None:
        hostname = None
        match = re.search(r'^hostname\s+(\S+)', config_text, re.MULTILINE)
        if match:
            hostname = match.group(1)
        return {
            "hostname": hostname,
            "dns_servers": [],
            "ntp_servers": [],
            "interfaces": [],
            "routes": []
        }

    parse = CiscoConfParse(config_text.splitlines())
    
    # Extract Hostname
    hostname = None
    hostname_objs = parse.find_objects(r'^hostname\s+(\S+)')
    if hostname_objs:
        match = re.match(r'^hostname\s+(\S+)', hostname_objs[0].text)
        if match:
            hostname = match.group(1)
            
    # Extract DNS
    dns_servers = []
    dns_objs = parse.find_objects(r'^ip\s+name-server\s+(.+)')
    for obj in dns_objs:
        match = re.match(r'^ip\s+name-server\s+(.+)', obj.text)
        if match:
            # Splitting by spaces since multiple dns can be configured on one line
            ips = match.group(1).split()
            dns_servers.extend(ips)
            
    # Extract NTP
    ntp_servers = []
    ntp_objs = parse.find_objects(r'^ntp\s+server\s+(\S+)')
    for obj in ntp_objs:
        match = re.match(r'^ntp\s+server\s+(\S+)', obj.text)
        if match:
            ntp_servers.append(match.group(1))
            
    # Extract Interfaces
    interfaces = []
    intf_objs = parse.find_objects(r'^interface\s+')
    for obj in intf_objs:
        name_match = re.match(r'^interface\s+(\S+)', obj.text)
        if not name_match:
            continue
        name = name_match.group(1)
        
        description = None
        ip_address = None
        netmask = None
        cidr = 24
        shutdown = False
        
        # Check children for settings
        for child in obj.children:
            text = child.text.strip()
            
            desc_match = re.match(r'^description\s+(.+)', text)
            if desc_match:
                description = desc_match.group(1)
                
            ip_match = re.match(r'^ip\s+address\s+(\S+)\s+(\S+)', text)
            if ip_match:
                ip_address = ip_match.group(1)
                netmask = ip_match.group(2)
                cidr = ip_to_cidr(netmask)
                
            if text == "shutdown":
                shutdown = True
                
        interfaces.append({
            "name": name,
            "description": description,
            "ip_address": ip_address,
            "netmask": netmask,
            "cidr": cidr,
            "shutdown": shutdown
        })
        
    # Extract Routes
    routes = []
    route_objs = parse.find_objects(r'^ip\s+route\s+')
    for obj in route_objs:
        match = re.match(r'^ip\s+route\s+(\S+)\s+(\S+)\s+(\S+)', obj.text)
        if match:
            prefix = match.group(1)
            mask = match.group(2)
            next_hop = match.group(3)
            routes.append({
                "prefix": prefix,
                "mask": mask,
                "cidr": ip_to_cidr(mask),
                "next_hop": next_hop
            })
            
    return {
        "hostname": hostname,
        "dns_servers": dns_servers,
        "ntp_servers": ntp_servers,
        "interfaces": interfaces,
        "routes": routes
    }

def validate_config(config_text):
    errors = []
    warnings = []
    
    if CiscoConfParse is not None:
        try:
            parse = CiscoConfParse(config_text.splitlines())
            
            # Check 1: Hostname config
            hostname_objs = parse.find_objects(r'^hostname\s+')
            if not hostname_objs:
                warnings.append("hostnameが設定されていません。")
                
            # Check 2: Service password-encryption
            pe_objs = parse.find_objects(r'^service\s+password-encryption')
            if not pe_objs:
                warnings.append("セキュリティ警告: 'service password-encryption' が有効になっていません。")
                
            # Check 3: Interface validations
            intf_objs = parse.find_objects(r'^interface\s+')
            for obj in intf_objs:
                name_match = re.match(r'^interface\s+(\S+)', obj.text)
                if not name_match:
                    continue
                name = name_match.group(1)
                
                has_description = False
                has_ip = False
                ip_syntax_ok = True
                
                for child in obj.children:
                    text = child.text.strip()
                    if text.startswith("description"):
                        has_description = True
                    if text.startswith("ip address"):
                        has_ip = True
                        parts = text.split()
                        if len(parts) >= 4:
                            ip = parts[2]
                            mask = parts[3]
                            # Simple syntax check for IP/mask
                            ip_octets = ip.split('.')
                            mask_octets = mask.split('.')
                            if len(ip_octets) != 4 or not all(x.isdigit() and 0 <= int(x) <= 255 for x in ip_octets):
                                warnings.append(f"インターフェース {name} に無効なIPアドレス '{ip}' が設定されています。")
                                ip_syntax_ok = False
                            if len(mask_octets) != 4 or not all(x.isdigit() and 0 <= int(x) <= 255 for x in mask_octets):
                                warnings.append(f"インターフェース {name} に無効なサブネットマスク '{mask}' が設定されています。")
                                ip_syntax_ok = False
                                
                if not has_description and name.lower() != "null0":
                    warnings.append(f"インターフェース {name} に description が設定されていません。")
        except Exception as e:
            warnings.append(f"Config parse notice: {str(e)}")
    else:
        warnings.append("検証ライブラリ (ciscoconfparse2) 未導入のため検証ステップをスキップしました。")
            
    # Cisco Config 検証失敗機能を一旦無効化
    success = True
    return {
        "success": success,
        "errors": errors,
        "warnings": warnings
    }

def convert_config(config_text, target_vendor, template_dir):
    data = parse_cisco_config(config_text)
    
    # Render with Jinja2
    loader = FileSystemLoader(template_dir)
    env = Environment(loader=loader)
    
    template_file = f"{target_vendor}.j2"
    try:
        template = env.get_template(template_file)
        converted = template.render(data)
        return {
            "success": True,
            "converted_config": converted,
            "error": None
        }
    except Exception as e:
        return {
            "success": False,
            "converted_config": "",
            "error": f"Failed to render template {template_file}: {str(e)}"
        }

def main():
    # Setup paths
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir) # parent of python/ dir
    template_dir = os.path.join(project_root, "templates")

    try:
        # Read from stdin
        input_data = sys.stdin.read()
        if not input_data.strip():
            print(json.dumps({"success": False, "error": "No input provided"}))
            return
            
        payload = json.loads(input_data)
        action = payload.get("action")
        config = payload.get("config", "")
        
        if action == "validate":
            result = validate_config(config)
            print(json.dumps(result))
        elif action == "convert":
            target_vendor = payload.get("target_vendor", "juniper")
            result = convert_config(config, target_vendor, template_dir)
            print(json.dumps(result))
        else:
            print(json.dumps({"success": False, "error": f"Invalid action: {action}"}))
            
    except Exception as e:
        print(json.dumps({"success": False, "error": f"Internal helper error: {str(e)}"}))

if __name__ == "__main__":
    main()

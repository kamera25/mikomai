"""
Netmiko Dynamic Extension (Monkey Patch) for Serial (Console) Connections.

This module dynamically registers custom serial connection classes in Netmiko's
internal CLASS_MAPPER and platforms list. It allows connecting to any supported
devices over serial console interfaces by dynamically generating '{device_type}_serial'
platforms.
"""

import logging
import netmiko
from netmiko.ssh_dispatcher import CLASS_MAPPER, platforms

# Configure logging
logger = logging.getLogger(__name__)

def apply_patches():
    """
    Apply monkey patches to Netmiko's ssh_dispatcher CLASS_MAPPER and platforms.
    This enables dynamic support for new '_serial' device types for ALL platforms.
    """
    serial_devices = {}

    # 1. Dynamically discover and register serial variants for all base platforms
    # We find base platform keys (keys that don't have suffix _ssh, _telnet, or _serial)
    base_platforms = [
        p for p in CLASS_MAPPER.keys()
        if not p.endswith("_telnet")
        and not p.endswith("_ssh")
        and not p.endswith("_serial")
        and p not in ("autodetect", "terminal_server")
    ]

    for base_platform in base_platforms:
        serial_type = f"{base_platform}_serial"
        # Skip if already registered (like cisco_ios_serial)
        if serial_type in CLASS_MAPPER:
            continue

        ssh_class = CLASS_MAPPER[base_platform]
        class_name = f"{ssh_class.__name__}Serial"
        if class_name.endswith("SSHSerial"):
            class_name = class_name.replace("SSHSerial", "Serial")

        # Dynamically create the serial class inheriting from the SSH connection class
        try:
            dynamic_serial_class = type(
                class_name,
                (ssh_class,),
                {"__doc__": f"Dynamic Serial connection class for {base_platform}."}
            )
            serial_devices[serial_type] = dynamic_serial_class
        except Exception as e:
            logger.error(f"Failed to create dynamic serial class for {base_platform}: {e}")

    # 2. Register all discovered/defined device types in CLASS_MAPPER
    for device_type, conn_class in serial_devices.items():
        if device_type not in CLASS_MAPPER:
            CLASS_MAPPER[device_type] = conn_class
            logger.debug(f"Registered device type '{device_type}' in CLASS_MAPPER.")
        else:
            logger.debug(f"Device type '{device_type}' is already registered in CLASS_MAPPER.")

    # 3. Update the global platforms list in netmiko.ssh_dispatcher
    global platforms
    for device_type in serial_devices:
        if device_type not in platforms:
            platforms.append(device_type)
    platforms.sort()

    # 4. Safely update netmiko.platforms (if it exists) to ensure sync
    if hasattr(netmiko, "platforms"):
        current_platforms = getattr(netmiko, "platforms")
        if isinstance(current_platforms, list):
            for device_type in serial_devices:
                if device_type not in current_platforms:
                    current_platforms.append(device_type)
            current_platforms.sort()
        elif isinstance(current_platforms, tuple):
            new_platforms = list(current_platforms)
            for device_type in serial_devices:
                if device_type not in new_platforms:
                    new_platforms.append(device_type)
            setattr(netmiko, "platforms", tuple(sorted(new_platforms)))

    logger.info(f"Successfully applied Netmiko serial connection patches. Registered {len(serial_devices)} serial platforms.")

# Automatically run the patches upon import of this module
apply_patches()

#!/usr/bin/env python3
"""Find Loxone Miniserver by MAC address using ARP scan.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import subprocess
import re
import socket
import platform


def find_ip_by_mac(target_mac: str) -> str | None:
    """Find IP address for a given MAC address using ARP."""
    target_mac = target_mac.lower().replace("-", ":")
    
    print(f"Looking for device with MAC: {target_mac}")
    
    # Get ARP table
    if platform.system() == "Darwin":  # macOS
        cmd = ["arp", "-a"]
    elif platform.system() == "Linux":
        cmd = ["arp", "-n"]
    else:
        print("Unsupported platform")
        return None
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True)
        
        # Parse ARP output
        for line in result.stdout.split("\n"):
            # macOS format: hostname (192.168.1.1) at aa:bb:cc:dd:ee:ff on en0
            # Linux format: 192.168.1.1  0x1  0x2  aa:bb:cc:dd:ee:ff  *  eth0
            
            if platform.system() == "Darwin":
                match = re.search(r'\(([0-9.]+)\) at ([0-9a-f:]+)', line.lower())
                if match:
                    ip, mac = match.groups()
                    if mac == target_mac:
                        return ip
            else:
                parts = line.split()
                if len(parts) >= 4:
                    ip = parts[0]
                    mac = parts[3].lower()
                    if mac == target_mac:
                        return ip
                        
    except Exception as e:
        print(f"Error running arp: {e}")
    
    return None


def test_loxone_connection(ip: str) -> bool:
    """Test if an IP responds like a Loxone Miniserver."""
    import httpx
    
    print(f"\nTesting connection to {ip}...")
    
    # Try common Loxone endpoints
    endpoints = [
        f"http://{ip}/jdev/cfg/api",  # API endpoint
        f"http://{ip}/jdev/sps/LoxAPPversion",  # Version info
        f"http://{ip}/",  # Root
    ]
    
    for endpoint in endpoints:
        try:
            response = httpx.get(endpoint, timeout=5.0, auth=None)
            print(f"  {endpoint}: {response.status_code}")
            
            if response.status_code == 401:
                print("    → Authentication required (this is a Loxone!)")
                return True
            elif response.status_code == 200:
                if "Loxone" in response.text or "LoxAPP" in response.text:
                    print("    → Loxone detected!")
                    return True
                    
        except Exception as e:
            print(f"  {endpoint}: Failed ({e})")
    
    return False


def resolve_hostname(hostname: str) -> str | None:
    """Try to resolve a hostname to IP."""
    variations = [
        hostname,
        f"{hostname}.local",
        hostname.replace("/", "-"),  # Try with dash instead of slash
        f"{hostname.replace('/', '-')}.local",
    ]
    
    for name in variations:
        try:
            ip = socket.gethostbyname(name)
            print(f"Resolved {name} to {ip}")
            return ip
        except socket.gaierror:
            continue
    
    return None


if __name__ == "__main__":
    target_mac = "50:4F:94:11:7E:95"
    
    # Try to find by MAC
    ip = find_ip_by_mac(target_mac)
    
    if ip:
        print(f"\n✓ Found device at IP: {ip}")
        if test_loxone_connection(ip):
            print(f"\n✅ Confirmed Loxone Miniserver at: http://{ip}")
            print(f"   You can update your credentials with: uvx --from . loxone-mcp setup")
    else:
        print("\n❌ Could not find device by MAC in ARP table")
        print("\nTrying hostname resolution...")
        
        # Try common Loxone hostname patterns
        hostnames = ["Beier/A5", "Beier-A5", "loxone", "miniserver"]
        
        for hostname in hostnames:
            ip = resolve_hostname(hostname)
            if ip:
                if test_loxone_connection(ip):
                    print(f"\n✅ Found Loxone at: http://{ip} (hostname: {hostname})")
                    break
        else:
            print("\n💡 Tips:")
            print("1. Make sure you're on the same network as the Miniserver")
            print("2. Try pinging the broadcast address: ping 192.168.178.255")
            print("3. Check your router's DHCP client list for the MAC")
            print("4. The Miniserver might use a static IP outside DHCP range")
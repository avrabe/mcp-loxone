#!/usr/bin/env python3
"""Discover Loxone Miniserver using Zeroconf/mDNS.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import socket
import time
from typing import Any

try:
    from zeroconf import ServiceBrowser, ServiceInfo, Zeroconf
    from zeroconf.asyncio import AsyncZeroconf
except ImportError:
    print("Please install zeroconf: pip install zeroconf")
    exit(1)


class LoxoneListener:
    """Listener for Zeroconf service discovery."""
    
    def __init__(self):
        self.services = []
    
    def add_service(self, zeroconf: Zeroconf, service_type: str, name: str) -> None:
        """Called when a service is discovered."""
        info = zeroconf.get_service_info(service_type, name)
        if info:
            print(f"\n✓ Found Loxone Miniserver!")
            print(f"  Name: {name}")
            print(f"  Type: {service_type}")
            
            # Parse addresses
            addresses = [socket.inet_ntoa(addr) for addr in info.addresses]
            for addr in addresses:
                print(f"  IP Address: {addr}")
            
            print(f"  Port: {info.port}")
            print(f"  Server: {info.server}")
            
            # Properties
            if info.properties:
                print("  Properties:")
                for key, value in info.properties.items():
                    if isinstance(value, bytes):
                        value = value.decode('utf-8', errors='ignore')
                    print(f"    {key}: {value}")
            
            # MAC address analysis
            if info.server:
                server_parts = info.server.split('.')
                if len(server_parts) > 0:
                    hostname = server_parts[0]
                    print(f"  Hostname: {hostname}")
                    
                    # Check if it looks like a Loxone hostname pattern
                    if '/' in hostname:
                        parts = hostname.split('/')
                        if len(parts) == 2:
                            print(f"    → Owner: {parts[0]}")
                            print(f"    → ID: {parts[1]}")
            
            self.services.append({
                'name': name,
                'addresses': addresses,
                'port': info.port,
                'server': info.server,
                'properties': info.properties
            })
    
    def remove_service(self, zeroconf: Zeroconf, service_type: str, name: str) -> None:
        """Called when a service is removed."""
        print(f"Service removed: {name}")
    
    def update_service(self, zeroconf: Zeroconf, service_type: str, name: str) -> None:
        """Called when a service is updated."""
        print(f"Service updated: {name}")


def discover_loxone_sync(timeout: float = 10.0) -> list[dict[str, Any]]:
    """Synchronous discovery of Loxone Miniservers."""
    print(f"Searching for Loxone Miniservers for {timeout} seconds...")
    print("Looking for service types:")
    print("  - _loxone._tcp.local.")
    print("  - _http._tcp.local.")
    print("  - _loxone-miniserver._tcp.local.")
    
    zeroconf = Zeroconf()
    listener = LoxoneListener()
    
    # Try multiple service types that Loxone might use
    browsers = []
    service_types = [
        "_loxone._tcp.local.",
        "_http._tcp.local.",
        "_loxone-miniserver._tcp.local.",
    ]
    
    for service_type in service_types:
        browser = ServiceBrowser(zeroconf, service_type, listener)
        browsers.append(browser)
    
    # Wait for discovery
    time.sleep(timeout)
    
    zeroconf.close()
    
    if not listener.services:
        print("\nNo Loxone Miniservers found.")
        print("\nTroubleshooting tips:")
        print("1. Make sure you're on the same network as the Miniserver")
        print("2. Check that mDNS/Bonjour is not blocked by firewall")
        print("3. Try the direct IP address if you know it")
        print("4. Your Miniserver hostname might be 'Beier/A5' based on MAC 50:4F:94:11:7E:95")
    
    return listener.services


async def discover_loxone_async(timeout: float = 10.0) -> list[dict[str, Any]]:
    """Asynchronous discovery of Loxone Miniservers."""
    print(f"Async searching for Loxone Miniservers for {timeout} seconds...")
    
    async with AsyncZeroconf() as azeroconf:
        listener = LoxoneListener()
        
        # Try multiple service types
        browsers = []
        service_types = [
            "_loxone._tcp.local.",
            "_http._tcp.local.",
            "_loxone-miniserver._tcp.local.",
        ]
        
        for service_type in service_types:
            browser = ServiceBrowser(azeroconf.zeroconf, service_type, listener)
            browsers.append(browser)
        
        # Wait for discovery
        await asyncio.sleep(timeout)
    
    return listener.services


def analyze_mac_address(mac: str) -> dict[str, Any]:
    """Analyze a MAC address to determine if it's a Loxone device."""
    # Loxone MAC prefixes (OUI - Organizationally Unique Identifier)
    loxone_ouis = [
        "50:4F:94",  # Loxone Electronics GmbH
        "00:0F:E5",  # Another possible Loxone OUI
    ]
    
    mac_upper = mac.upper().replace("-", ":")
    oui = ":".join(mac_upper.split(":")[:3])
    
    is_loxone = oui in loxone_ouis
    
    return {
        "mac": mac,
        "oui": oui,
        "is_loxone": is_loxone,
        "vendor": "Loxone Electronics GmbH" if is_loxone else "Unknown"
    }


if __name__ == "__main__":
    # Analyze the provided MAC address
    mac_info = analyze_mac_address("50:4F:94:11:7E:95")
    print("MAC Address Analysis:")
    print(f"  MAC: {mac_info['mac']}")
    print(f"  OUI: {mac_info['oui']}")
    print(f"  Vendor: {mac_info['vendor']}")
    print(f"  Is Loxone: {'Yes' if mac_info['is_loxone'] else 'No'}")
    print()
    
    # Try synchronous discovery
    services = discover_loxone_sync(timeout=10.0)
    
    if services:
        print(f"\nFound {len(services)} Loxone Miniserver(s)")
        
        # Based on your MAC, the hostname might be "Beier/A5"
        print("\nBased on MAC 50:4F:94:11:7E:95, your Miniserver might be:")
        print("  Hostname: Beier/A5.local")
        print("  Try accessing: http://Beier/A5.local or http://192.168.x.x")
    else:
        print("\nAlternative: Try async discovery...")
        # Try async discovery
        loop = asyncio.get_event_loop()
        services = loop.run_until_complete(discover_loxone_async(timeout=10.0))
        
        if services:
            print(f"\nAsync found {len(services)} Loxone Miniserver(s)")
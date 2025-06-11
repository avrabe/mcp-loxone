"""Secure credential management using system keychain.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import getpass
import json
import logging
import os
import secrets
import socket
import sys

import httpx
import keyring

logger = logging.getLogger(__name__)


class LoxoneSecrets:
    """Manages Loxone credentials using the system keychain."""

    SERVICE_NAME = "LoxoneMCP"

    # Credential keys
    HOST_KEY = "LOXONE_HOST"
    USER_KEY = "LOXONE_USER"
    PASS_KEY = "LOXONE_PASS"
    SSE_API_KEY = "LOXONE_SSE_API_KEY"

    @classmethod
    def get(cls, key: str) -> str | None:
        """
        Retrieve a secret from environment variables or system keychain.

        Environment variables take precedence for CI/CD compatibility.

        Args:
            key: The credential key to retrieve

        Returns:
            The credential value or None if not found
        """
        # First check environment variables
        value = os.getenv(key)
        if value:
            return value

        # Then check system keychain
        try:
            return keyring.get_password(cls.SERVICE_NAME, key)
        except Exception as e:
            print(f"Warning: Could not access keychain: {e}", file=sys.stderr)
            return None

    @classmethod
    def set(cls, key: str, value: str) -> None:
        """Store a secret in the system keychain."""
        try:
            keyring.set_password(cls.SERVICE_NAME, key, value)
        except Exception as e:
            print(f"Error: Could not store credential in keychain: {e}", file=sys.stderr)
            raise

    @classmethod
    def delete(cls, key: str) -> None:
        """Remove a secret from the system keychain."""
        try:
            keyring.delete_password(cls.SERVICE_NAME, key)
        except keyring.errors.PasswordDeleteError:
            pass  # Already deleted
        except Exception as e:
            print(f"Warning: Could not delete credential: {e}", file=sys.stderr)

    @classmethod
    def generate_api_key(cls) -> str:
        """Generate a secure API key for SSE authentication."""
        return secrets.token_urlsafe(32)

    @classmethod
    async def discover_loxone_servers(cls, timeout: float = 5.0) -> list[dict[str, str]]:
        """Discover Loxone Miniservers on the local network using multiple methods."""
        servers = []

        print("🔍 Discovering Loxone Miniservers on your network...")

        # Method 1: UDP Discovery (Loxone specific protocol)
        print("   • Trying UDP discovery...", end=" ", flush=True)
        udp_servers = await cls._udp_discovery(timeout=2.0)
        if udp_servers:
            print(f"✅ Found {len(udp_servers)} server(s)")
            servers.extend(udp_servers)
        else:
            print("⏭️  No response")

        # Method 2: Network scan for HTTP endpoints
        print("   • Scanning network for HTTP endpoints...", end=" ", flush=True)
        http_servers = await cls._http_discovery(timeout=max(1.0, timeout - 2.0))

        # Merge results, avoiding duplicates
        existing_ips = {s["ip"] for s in servers}
        new_servers = []
        for server in http_servers:
            if server["ip"] not in existing_ips:
                servers.append(server)
                new_servers.append(server)

        if new_servers:
            print(f"✅ Found {len(new_servers)} additional server(s)")
        elif not udp_servers:
            print("❌ No servers found")
        else:
            print("⏭️  No additional servers")

        # Sort servers by IP for consistent ordering
        servers.sort(key=lambda x: tuple(map(int, x["ip"].split("."))))

        return servers

    @classmethod
    async def _udp_discovery(cls, timeout: float = 2.0) -> list[dict[str, str]]:
        """Discover Loxone servers using UDP broadcast."""
        servers = []

        try:
            # Create UDP socket for discovery
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
            sock.settimeout(timeout)

            # Loxone discovery message (varies by version, try common ones)
            discovery_messages = [
                b"LoxLIVE",  # Common discovery message
                b"eWeLink",  # Alternative discovery
                b"\x00\x00\x00\x00",  # Simple broadcast
            ]

            # Send discovery packets to common ports
            for port in [7777, 7700, 80, 8080]:
                for msg in discovery_messages:
                    try:
                        sock.sendto(msg, ("<broadcast>", port))
                    except Exception:
                        continue

            # Listen for responses
            start_time = asyncio.get_event_loop().time()
            responses = []

            while asyncio.get_event_loop().time() - start_time < timeout:
                try:
                    data, addr = sock.recvfrom(1024)
                    if addr[0] not in [r[1][0] for r in responses]:
                        responses.append((data, addr))
                except TimeoutError:
                    break
                except Exception:
                    continue

            sock.close()

            # Process responses
            for data, addr in responses:
                try:
                    # Try to parse as JSON
                    if data.startswith(b"{"):
                        info = json.loads(data.decode())
                        name = info.get("name", "Loxone Miniserver")
                    else:
                        name = "Loxone Miniserver"

                    servers.append(
                        {"ip": addr[0], "name": name, "port": "80", "method": "UDP Discovery"}
                    )
                except Exception:
                    # Even if we can't parse the response, it's likely a Loxone device
                    servers.append(
                        {
                            "ip": addr[0],
                            "name": "Loxone Miniserver",
                            "port": "80",
                            "method": "UDP Discovery",
                        }
                    )

        except Exception as e:
            logger.debug(f"UDP discovery error: {e}")

        return servers

    @classmethod
    async def _http_discovery(cls, timeout: float = 3.0) -> list[dict[str, str]]:
        """Discover Loxone servers by scanning network for HTTP endpoints."""
        servers = []

        try:
            # Get local network range
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.connect(("8.8.8.8", 80))
            local_ip = s.getsockname()[0]
            s.close()

            # Extract network prefix (e.g., 192.168.1.x -> 192.168.1)
            ip_parts = local_ip.split(".")
            network_prefix = ".".join(ip_parts[:3])

            # Scan common IP ranges
            async def check_ip(ip: str) -> dict[str, str] | None:
                try:
                    async with httpx.AsyncClient(timeout=0.5) as client:
                        # First check if port 80 responds
                        response = await client.get(f"http://{ip}/", follow_redirects=False)

                        # Check if it's likely a Loxone (returns 401 for unauthorized access)
                        if response.status_code in [401, 200]:
                            # Try to get Miniserver info
                            name = "Loxone Miniserver"
                            version = "Unknown"

                            try:
                                # Try to get version info without auth
                                info_response = await client.get(f"http://{ip}/jdev/sys/getversion")
                                if info_response.status_code == 200:
                                    data = info_response.json()
                                    version = data.get("LL", {}).get("value", "Unknown")

                                # Try to get project name (might require auth)
                                cfg_response = await client.get(f"http://{ip}/jdev/cfg/api")
                                if cfg_response.status_code == 200:
                                    data = cfg_response.json()
                                    name = data.get("LL", {}).get("value", {}).get("name", name)
                            except Exception:
                                pass

                            return {
                                "ip": ip,
                                "name": f"{name} (v{version})" if version != "Unknown" else name,
                                "port": "80",
                                "method": "HTTP Scan",
                            }
                except Exception:
                    pass
                return None

            # Scan common IP ranges (limit to reasonable subset for speed)
            tasks = []
            # Check common router/device IPs first
            priority_ips = [
                f"{network_prefix}.{i}" for i in [1, 2, 10, 100, 101, 102, 200, 201, 202]
            ]
            for ip in priority_ips:
                tasks.append(check_ip(ip))

            # Then scan broader range
            for i in range(3, 255):
                ip = f"{network_prefix}.{i}"
                if ip not in priority_ips:
                    tasks.append(check_ip(ip))

            # Wait for results with timeout
            try:
                results = await asyncio.wait_for(
                    asyncio.gather(*tasks, return_exceptions=True), timeout=timeout
                )
                servers = [s for s in results if s is not None and isinstance(s, dict)]
            except TimeoutError:
                logger.debug("HTTP discovery timed out")

        except Exception as e:
            logger.debug(f"HTTP discovery error: {e}")

        return servers

    @classmethod
    async def _test_connection(cls, host: str, username: str, password: str) -> dict[str, any]:
        """Test connection to Loxone Miniserver."""
        try:
            async with httpx.AsyncClient(timeout=5.0) as client:
                # Try to get structure file with credentials
                response = await client.get(
                    f"http://{host}/data/LoxAPP3.json", auth=(username, password)
                )

                if response.status_code == 200:
                    # Success! Try to get some info
                    try:
                        data = response.json()
                        info = {
                            "name": data.get("msInfo", {}).get("projectName", "Unknown"),
                            "version": data.get("msInfo", {}).get("swVersion", "Unknown"),
                        }
                        return {"success": True, "info": info}
                    except Exception:
                        return {"success": True}
                elif response.status_code == 401:
                    return {"success": False, "error": "Invalid username or password"}
                else:
                    return {"success": False, "error": f"HTTP {response.status_code}"}
        except httpx.ConnectError:
            return {"success": False, "error": "Cannot connect to Miniserver"}
        except httpx.TimeoutException:
            return {"success": False, "error": "Connection timeout"}
        except Exception as e:
            return {"success": False, "error": str(e)}

    @classmethod
    def setup(cls) -> None:
        """Interactive setup wizard for configuring Loxone credentials."""
        print("🔐 Loxone MCP Server Setup")
        print("=" * 40)

        # Try to discover Loxone servers first
        discovered_servers = asyncio.run(cls.discover_loxone_servers())

        host = None
        if discovered_servers:
            print(f"\n✅ Found {len(discovered_servers)} Loxone Miniserver(s) on your network:\n")
            for i, server in enumerate(discovered_servers, 1):
                method = server.get("method")
                method_info = f" ({method})" if method else ""
                print(f"  {i}. {server['name']} at {server['ip']}{method_info}")

            print(f"\n  {len(discovered_servers) + 1}. Enter IP address manually")
            print("\n  0. Cancel setup")

            while True:
                max_option = len(discovered_servers) + 1
                choice = input(f"\nSelect an option (1-{max_option}, or 0 to cancel): ").strip()

                if choice == "0":
                    print("Setup cancelled.")
                    return
                elif choice.isdigit():
                    choice_num = int(choice)
                    if 1 <= choice_num <= len(discovered_servers):
                        selected = discovered_servers[choice_num - 1]
                        host = selected["ip"]
                        print(f"\n✅ Selected: {selected['name']} at {host}")
                        break
                    elif choice_num == len(discovered_servers) + 1:
                        # User wants to enter manually
                        break
                    else:
                        max_choice = len(discovered_servers) + 1
                        print(
                            f"Invalid choice. Please enter a number between 1 and {max_choice}, "
                            "or 0 to cancel."
                        )
                else:
                    print("Please enter a valid number.")
        else:
            print("\n❌ No Loxone Miniservers found on the network.")
            print("   This could happen if:")
            print("   • Your Miniserver is on a different network segment")
            print("   • The Miniserver is using a non-standard port")
            print("   • Firewall is blocking discovery")
            print("\n   You can still enter the IP address manually below.")

        print("\nThis wizard will securely store your Loxone credentials")
        print("in your system keychain.\n")

        # Check for existing credentials
        existing = cls.get(cls.HOST_KEY) is not None
        if existing:
            response = input("Credentials already exist. Replace them? [y/N]: ")
            if response.lower() != "y":
                print("Setup cancelled.")
                return
            print()

        # Collect credentials
        print("Please enter your Loxone Miniserver details:\n")

        # If no host was selected from discovery, ask for it
        if not host:
            host = input("Miniserver IP address (e.g., 192.168.1.100): ").strip()
            if not host:
                print("Error: Host cannot be empty")
                sys.exit(1)

        username = input("Username: ").strip()
        if not username:
            print("Error: Username cannot be empty")
            sys.exit(1)

        password = getpass.getpass("Password: ")
        if not password:
            print("Error: Password cannot be empty")
            sys.exit(1)

        # Test connection before saving
        print("\n🔌 Testing connection...")
        test_result = asyncio.run(cls._test_connection(host, username, password))

        if not test_result["success"]:
            print(f"\n❌ Connection failed: {test_result['error']}")
            retry = input("\nWould you like to try again? [Y/n]: ")
            if retry.lower() != "n":
                cls.setup()  # Restart setup
                return
            else:
                sys.exit(1)

        print("\n✅ Successfully connected to Loxone Miniserver!")
        if test_result.get("info"):
            print(f"   Miniserver: {test_result['info'].get('name', 'Unknown')}")
            print(f"   Version: {test_result['info'].get('version', 'Unknown')}")

        # Store credentials
        try:
            cls.set(cls.HOST_KEY, host)
            cls.set(cls.USER_KEY, username)
            cls.set(cls.PASS_KEY, password)

            print("\n✅ Credentials stored successfully!")
            print(f"   Host: {host}")
            print(f"   User: {username}")
            print(f"   Pass: {'*' * len(password)}")

        except Exception as e:
            print(f"\n❌ Error storing credentials: {e}")
            sys.exit(1)

        # Setup SSE API key for web integrations
        print("\n🌐 SSE Server Setup (for web integrations like n8n, Home Assistant)")
        print("=" * 60)

        existing_api_key = cls.get(cls.SSE_API_KEY)
        if existing_api_key:
            print(f"✅ SSE API key already configured: {existing_api_key[:8]}...")
            replace_key = input("Replace existing API key? [y/N]: ").strip().lower()
            if replace_key != "y":
                print("   Keeping existing API key")
            else:
                existing_api_key = None

        if not existing_api_key:
            print("\nChoose SSE API key setup:")
            print("  1. Generate secure API key automatically (recommended)")
            print("  2. Enter custom API key")
            print("  3. Skip SSE setup (can be configured later)")

            while True:
                choice = input("\nSelect option [1-3]: ").strip()

                if choice == "1":
                    # Generate API key
                    api_key = cls.generate_api_key()
                    try:
                        cls.set(cls.SSE_API_KEY, api_key)
                        print("\n🔑 Generated and stored SSE API key!")
                        print(f"   API Key: {api_key}")
                        print("\n📋 Use this for web integrations:")
                        print(f"   Authorization: Bearer {api_key}")
                        print(f"   OR X-API-Key: {api_key}")
                        break
                    except Exception as e:
                        print(f"❌ Error storing API key: {e}")
                        sys.exit(1)

                elif choice == "2":
                    # Custom API key
                    api_key = input("Enter your custom API key: ").strip()
                    if not api_key:
                        print("❌ API key cannot be empty")
                        continue
                    if len(api_key) < 16:
                        print("⚠️  Warning: API key should be at least 16 characters for security")
                        confirm = input("Continue anyway? [y/N]: ").strip().lower()
                        if confirm != "y":
                            continue

                    try:
                        cls.set(cls.SSE_API_KEY, api_key)
                        print("\n✅ Custom API key stored!")
                        print(f"   API Key: {api_key}")
                        break
                    except Exception as e:
                        print(f"❌ Error storing API key: {e}")
                        sys.exit(1)

                elif choice == "3":
                    # Skip SSE setup
                    print("⏭️  SSE setup skipped")
                    print("   You can generate an API key later by:")
                    print("   1. Running setup again, or")
                    print("   2. Setting LOXONE_SSE_API_KEY environment variable")
                    break

                else:
                    print("❌ Invalid choice. Please enter 1, 2, or 3.")

        # Summary and next steps
        print("\n📝 Next steps:")
        print("1. Test MCP server: uv run mcp dev src/loxone_mcp/server.py")
        print("2. Test SSE server: uvx --from . loxone-mcp-sse")
        print("3. Configure in Claude Desktop (see README.md)")

        if cls.get(cls.SSE_API_KEY):
            print("4. Use API key for web integrations (n8n, Home Assistant)")
        else:
            print("4. Configure SSE API key later if needed for web integrations")

    @classmethod
    def clear_all(cls) -> None:
        """Remove all stored credentials."""
        for key in [cls.HOST_KEY, cls.USER_KEY, cls.PASS_KEY, cls.SSE_API_KEY]:
            cls.delete(key)
        print("✅ All credentials cleared")

    @classmethod
    def validate(cls) -> bool:
        """Check if all required credentials are available."""
        required = [cls.HOST_KEY, cls.USER_KEY, cls.PASS_KEY]
        missing = [key for key in required if not cls.get(key)]

        if missing:
            print(f"❌ Missing credentials: {', '.join(missing)}")
            print("Run 'uvx --from . loxone-mcp setup' to configure")
            return False

        # Check SSE API key (optional but warn if missing)
        if not cls.get(cls.SSE_API_KEY):
            print("⚠️  SSE API key not configured - SSE server will generate one automatically")
            print("   For production use, run setup again or set LOXONE_SSE_API_KEY")

        return True


if __name__ == "__main__":
    # Allow running this file directly for setup
    if len(sys.argv) > 1:
        if sys.argv[1] == "setup":
            LoxoneSecrets.setup()
        elif sys.argv[1] == "clear":
            LoxoneSecrets.clear_all()
        else:
            print(f"Unknown command: {sys.argv[1]}")
            print("Usage: python credentials.py [setup|clear]")
    else:
        # Validate existing credentials
        if LoxoneSecrets.validate():
            print("✅ All credentials are configured")

"""Secure credential management with Infisical integration and keychain fallback.

This module provides backward-compatible credential management that can use:
1. Infisical (when configured) for team/production environments
2. System keychain for local development
3. Environment variables for CI/CD

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
from typing import Any

import httpx
import keyring

logger = logging.getLogger(__name__)

# Import the enhanced Infisical-based credential system
try:
    from .infisical_credentials import InfisicalSecrets as _InfisicalSecrets
    _INFISICAL_AVAILABLE = True
except ImportError:
    _INFISICAL_AVAILABLE = False
    _InfisicalSecrets = None


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

        # Method 1: Zeroconf/mDNS Discovery (most accurate)
        print("   • Trying mDNS/zeroconf discovery...", end=" ", flush=True)
        zeroconf_servers = await cls._zeroconf_discovery(timeout=3.0)
        if zeroconf_servers:
            print(f"✅ Found {len(zeroconf_servers)} server(s)")
            servers.extend(zeroconf_servers)
        else:
            print("⏭️  No mDNS announcements")

        # Method 2: UDP Discovery (Loxone specific protocol)
        print("   • Trying UDP discovery...", end=" ", flush=True)
        udp_servers = await cls._udp_discovery(timeout=2.0)
        if udp_servers:
            print(f"✅ Found {len(udp_servers)} server(s)")
            servers.extend(udp_servers)
        else:
            print("⏭️  No UDP response")

        # Method 3: Network scan for HTTP endpoints
        print("   • Scanning network for HTTP endpoints...", end=" ", flush=True)
        http_servers = await cls._http_discovery(timeout=max(1.0, timeout - 3.0))

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
    async def _zeroconf_discovery(cls, timeout: float = 3.0) -> list[dict[str, str]]:
        """Discover Loxone servers using Zeroconf/mDNS."""
        servers = []

        try:
            from zeroconf import ServiceBrowser, ServiceListener, Zeroconf

            class LoxoneServiceListener(ServiceListener):
                def __init__(self) -> None:
                    self.services = []

                def add_service(self, zc: Zeroconf, type_: str, name: str) -> None:
                    info = zc.get_service_info(type_, name)
                    if info and info.addresses:
                        # Convert IPv4 address to string
                        ip = socket.inet_ntoa(info.addresses[0])
                        port = info.port or 80

                        # Extract server name from service name or properties
                        server_name = "Loxone Miniserver"
                        if info.properties:
                            # Look for common Loxone properties
                            props = {k.decode(): v.decode() if isinstance(v, bytes) else str(v)
                                   for k, v in info.properties.items()}
                            server_name = props.get('name', props.get('friendlyName', server_name))

                        self.services.append({
                            "ip": ip,
                            "name": server_name,
                            "port": str(port),
                            "method": "mDNS/Zeroconf",
                            "service_type": type_,
                            "service_name": name
                        })

                def remove_service(self, zc: Zeroconf, type_: str, name: str) -> None:
                    pass

                def update_service(self, zc: Zeroconf, type_: str, name: str) -> None:
                    pass

            # Initialize zeroconf
            zc = Zeroconf()
            listener = LoxoneServiceListener()

            # Common service types that Loxone devices might announce
            service_types = [
                "_http._tcp.local.",      # Generic HTTP service
                "_loxone._tcp.local.",    # Loxone specific (if they use it)
                "_miniserver._tcp.local.", # Miniserver specific
                "_webdav._tcp.local.",    # WebDAV (sometimes used by Loxone)
                "_device-info._tcp.local." # Device info
            ]

            browsers = []
            for service_type in service_types:
                try:
                    browser = ServiceBrowser(zc, service_type, listener)
                    browsers.append(browser)
                except Exception:
                    continue

            # Wait for discoveries
            await asyncio.sleep(timeout)

            # Cleanup
            for browser in browsers:
                browser.cancel()
            zc.close()

            # Filter results to likely Loxone devices
            for service in listener.services:
                name_lower = service["name"].lower()
                if (
                    any(term in name_lower for term in ["loxone", "miniserver", "lox"])
                    or service["service_type"] == "_loxone._tcp.local."
                ):
                    servers.append(service)

        except ImportError:
            logger.debug("Zeroconf not available, skipping mDNS discovery")
        except Exception as e:
            logger.debug(f"Zeroconf discovery error: {e}")

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
    def setup(
        cls,
        host: str | None = None,
        username: str | None = None,
        password: str | None = None,
        api_key: str | None = None,
        enable_discovery: bool = True,
        discovery_timeout: float = 5.0,
        interactive: bool = True,
    ) -> None:
        """Setup wizard for configuring Loxone credentials with Infisical support.

        Args:
            host: Miniserver IP address (if provided, skips discovery)
            username: Username for authentication
            password: Password for authentication
            api_key: SSE API key (optional)
            enable_discovery: Whether to run server discovery
            discovery_timeout: Timeout for discovery in seconds
            interactive: Whether to run in interactive mode
        """
        print("🔐 Loxone MCP Server Setup")
        print("=" * 40)

        # Check if we're using the new Infisical-based system
        from .infisical_credentials import InfisicalSecrets
        infisical_manager = InfisicalSecrets()
        infisical_manager.show_configuration_info()

        print("\n💡 Credential Storage Options:")
        print("  1. Infisical (recommended for teams) - Configure via environment variables")
        print("  2. Environment variables (good for CI/CD)")
        print("  3. System keychain (individual use)")

        # Determine which backend we're using
        if infisical_manager.project_id:
            print("\n✅ Infisical configuration detected - will store credentials in Infisical")
            credential_manager = infisical_manager
        else:
            print("\n💡 Using environment variables or keychain storage")
            credential_manager = cls()

        # Server discovery (only if enabled and host not provided)
        discovered_servers = []
        if enable_discovery and not host:
            discovered_servers = asyncio.run(cls.discover_loxone_servers(timeout=discovery_timeout))
        elif host:
            print(f"📍 Using provided host: {host}")
        elif not enable_discovery:
            print("🚫 Server discovery disabled")

        # Server selection logic
        if not host:  # Only if host wasn't provided via CLI
            if discovered_servers:
                print(f"\n✅ Found {len(discovered_servers)} Loxone Miniserver(s) on your network:")
                for i, server in enumerate(discovered_servers, 1):
                    method = server.get("method")
                    method_info = f" ({method})" if method else ""
                    print(f"  {i}. {server['name']} at {server['ip']}{method_info}")

                if interactive:
                    print(f"\n  {len(discovered_servers) + 1}. Enter IP address manually")
                    print("\n  0. Cancel setup")

                    while True:
                        max_option = len(discovered_servers) + 1
                        choice = input(
                            f"\nSelect an option (1-{max_option}, or 0 to cancel): "
                        ).strip()

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
                                    f"Invalid choice. Please enter a number between 1 and "
                                    f"{max_choice}, or 0 to cancel."
                                )
                        else:
                            print("Please enter a valid number.")
                else:
                    # Non-interactive: automatically select first discovered server
                    selected = discovered_servers[0]
                    host = selected["ip"]
                    print(f"\n🤖 Non-interactive mode: Auto-selected {selected['name']} at {host}")
        else:
            print("\n❌ No Loxone Miniservers found on the network.")
            print("   This could happen if:")
            print("   • Your Miniserver is on a different network segment")
            print("   • The Miniserver is using a non-standard port")
            print("   • Firewall is blocking discovery")
            print("\n   You can still enter the IP address manually below.")

        backend_name = (
            "Infisical" if infisical_manager.project_id
            else "your chosen storage backend"
        )
        print(f"\nThis wizard will securely store your Loxone credentials in {backend_name}.\n")

        # Check for existing credentials
        existing = credential_manager.get(credential_manager.HOST_KEY) is not None
        if existing:
            if interactive:
                response = input("Credentials already exist. Replace them? [y/N]: ")
                if response.lower() != "y":
                    print("Setup cancelled.")
                    return
                print()
            else:
                print("🔄 Replacing existing credentials (non-interactive mode)")

        # Collect credentials
        if interactive:
            print("Please enter your Loxone Miniserver details:\n")

        # Validate required parameters for non-interactive mode
        if not interactive:
            if not host:
                print("❌ Error: --host required in non-interactive mode")
                sys.exit(1)
            if not username:
                print("❌ Error: --username required in non-interactive mode")
                sys.exit(1)
            if not password:
                print("❌ Error: --password required in non-interactive mode")
                sys.exit(1)

        # Collect host (if not already set)
        if not host:
            if interactive:
                host = input("Miniserver IP address (e.g., 192.168.1.100): ").strip()
                if not host:
                    print("Error: Host cannot be empty")
                    sys.exit(1)
            else:
                print("❌ Error: Host not available from discovery or CLI arguments")
                sys.exit(1)

        # Collect username
        if not username and interactive:
                username = input("Username: ").strip()
                if not username:
                    print("Error: Username cannot be empty")
                    sys.exit(1)

        # Collect password
        if not password and interactive:
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

        # Store credentials using the selected manager
        try:
            credential_manager.set(credential_manager.HOST_KEY, host)
            credential_manager.set(credential_manager.USER_KEY, username)
            credential_manager.set(credential_manager.PASS_KEY, password)

            storage_location = (
                "Infisical" if infisical_manager.project_id
                else "keychain/environment"
            )
            print(f"\n✅ Credentials stored successfully in {storage_location}!")
            print(f"   Host: {host}")
            print(f"   User: {username}")
            print(f"   Pass: {'*' * len(password)}")

        except Exception as e:
            print(f"\n❌ Error storing credentials: {e}")
            sys.exit(1)

        # Setup SSE API key for web integrations
        print("\n🌐 SSE Server Setup (for web integrations like n8n, Home Assistant)")
        print("=" * 60)

        existing_api_key = credential_manager.get(credential_manager.SSE_API_KEY)
        if existing_api_key:
            print(f"✅ SSE API key already configured: {existing_api_key[:8]}...")
            if interactive:
                replace_key = input("Replace existing API key? [y/N]: ").strip().lower()
                if replace_key != "y":
                    print("   Keeping existing API key")
                else:
                    existing_api_key = None
            else:
                if api_key:
                    print("🔄 Replacing existing API key (non-interactive mode)")
                    existing_api_key = None
                else:
                    print("   Keeping existing API key (non-interactive mode)")

        if not existing_api_key:
            if interactive:
                print("\nChoose SSE API key setup:")
                print("  1. Generate secure API key automatically (recommended)")
                print("  2. Enter custom API key")
                print("  3. Skip SSE setup (can be configured later)")

                while True:
                    choice = input("\nSelect option [1-3]: ").strip()

                    if choice == "1":
                        # Generate API key
                        generated_api_key = cls.generate_api_key()
                        try:
                            credential_manager.set(
                                credential_manager.SSE_API_KEY, generated_api_key
                            )
                            print("\n🔑 Generated and stored SSE API key!")
                            print(f"   API Key: {generated_api_key}")
                            print("\n📋 Use this for web integrations:")
                            print(f"   Authorization: Bearer {generated_api_key}")
                            print(f"   OR X-API-Key: {generated_api_key}")
                            break
                        except Exception as e:
                            print(f"❌ Error storing API key: {e}")
                            sys.exit(1)

                    elif choice == "2":
                        # Custom API key
                        custom_api_key = input("Enter your custom API key: ").strip()
                        if not custom_api_key:
                            print("❌ API key cannot be empty")
                            continue
                        if len(custom_api_key) < 16:
                            print(
                                "⚠️  Warning: API key should be at least 16 characters for security"
                            )
                            confirm = input("Continue anyway? [y/N]: ").strip().lower()
                            if confirm != "y":
                                continue

                        try:
                            credential_manager.set(credential_manager.SSE_API_KEY, custom_api_key)
                            print("\n✅ Custom API key stored!")
                            print(f"   API Key: {custom_api_key}")
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
            else:
                # Non-interactive mode
                if api_key:
                    # Use provided API key
                    if len(api_key) < 16:
                        print("⚠️  Warning: Provided API key is shorter than 16 characters")
                    try:
                        credential_manager.set(credential_manager.SSE_API_KEY, api_key)
                        print(f"🔑 SSE API key stored (non-interactive): {api_key[:8]}...")
                    except Exception as e:
                        print(f"❌ Error storing API key: {e}")
                        sys.exit(1)
                else:
                    # Generate API key automatically in non-interactive mode
                    generated_api_key = cls.generate_api_key()
                    try:
                        credential_manager.set(credential_manager.SSE_API_KEY, generated_api_key)
                        print(f"🔑 Auto-generated SSE API key: {generated_api_key}")
                        print("📋 Use this for web integrations:")
                        print(f"   Authorization: Bearer {generated_api_key}")
                    except Exception as e:
                        print(f"❌ Error storing auto-generated API key: {e}")
                        sys.exit(1)

        # Summary and next steps
        print("\n📝 Next steps:")
        print("1. Test MCP server: uv run mcp dev src/loxone_mcp/server.py")
        print("2. Test SSE server: uvx --from . loxone-mcp-sse")
        print("3. Configure in Claude Desktop (see README.md)")

        if credential_manager.get(credential_manager.SSE_API_KEY):
            print("4. Use API key for web integrations (n8n, Home Assistant)")
        else:
            print("4. Configure SSE API key later if needed for web integrations")

        # Infisical-specific instructions
        if infisical_manager.project_id:
            print("\n🔐 Infisical Setup Complete!")
            print("   ✅ Credentials are now stored in your Infisical project")
            print("   ✅ Team members can access the same credentials")
            print("   💡 To share with team: provide them with the same environment variables:")
            print(f"      INFISICAL_PROJECT_ID={infisical_manager.project_id}")
            print(f"      INFISICAL_ENVIRONMENT={infisical_manager.environment}")
            print("      INFISICAL_CLIENT_ID=<their-client-id>")
            print("      INFISICAL_CLIENT_SECRET=<their-client-secret>")
        else:
            print("\n💡 To upgrade to team-friendly Infisical storage:")
            print("   1. Sign up at https://app.infisical.com")
            print("   2. Create a project and set up Universal Auth")
            print("   3. Set environment variables and run setup again")

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


def get_credentials_manager(**kwargs: Any) -> LoxoneSecrets:
    """Factory function to get the best available credential manager.

    Returns InfisicalSecrets if available and configured, otherwise LoxoneSecrets.
    """
    if _INFISICAL_AVAILABLE:
        # Check if Infisical is configured
        infisical_manager = _InfisicalSecrets(**kwargs)
        if infisical_manager.project_id:
            logger.info("Using Infisical-based credential management")
            return infisical_manager
        else:
            logger.info(
                "Infisical available but not configured, "
                "using enhanced system with keychain fallback"
            )
            return infisical_manager

    logger.info("Using traditional keychain-based credential management")
    return LoxoneSecrets()


if __name__ == "__main__":
    # Allow running this file directly for setup
    if len(sys.argv) > 1:
        # Use the enhanced credential manager if available
        manager = get_credentials_manager()

        if sys.argv[1] == "setup":
            manager.setup()
        elif sys.argv[1] == "clear":
            manager.clear_all()
        elif sys.argv[1] == "migrate" and hasattr(manager, 'migrate_from_keychain'):
            manager.migrate_from_keychain()
        else:
            print(f"Unknown command: {sys.argv[1]}")
            print("Usage: python credentials.py [setup|clear|migrate]")
    else:
        # Validate existing credentials
        manager = get_credentials_manager()
        if manager.validate():
            print("✅ All credentials are configured")

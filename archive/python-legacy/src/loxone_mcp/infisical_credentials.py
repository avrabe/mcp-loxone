"""Infisical-based credential management with keychain fallback.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import getpass
import logging
import os
import secrets
import sys
from enum import Enum
from typing import Any

logger = logging.getLogger(__name__)


class CredentialBackend(Enum):
    """Available credential storage backends."""

    ENVIRONMENT = "environment"
    INFISICAL = "infisical"
    KEYCHAIN = "keychain"


class InfisicalSecrets:
    """Enhanced credential management using Infisical with fallback strategies."""

    # Service name for backward compatibility
    SERVICE_NAME = "LoxoneMCP"

    # Credential keys (maintaining compatibility)
    HOST_KEY = "LOXONE_HOST"
    USER_KEY = "LOXONE_USER"
    PASS_KEY = "LOXONE_PASS"
    SSE_API_KEY = "LOXONE_SSE_API_KEY"

    # Infisical configuration keys
    INFISICAL_PROJECT_ID = "INFISICAL_PROJECT_ID"
    INFISICAL_ENVIRONMENT = "INFISICAL_ENVIRONMENT"
    INFISICAL_CLIENT_ID = "INFISICAL_CLIENT_ID"
    INFISICAL_CLIENT_SECRET = "INFISICAL_CLIENT_SECRET"

    def __init__(
        self,
        project_id: str | None = None,
        environment: str = "dev",
        client_id: str | None = None,
        client_secret: str | None = None,
        backend_preference: list[CredentialBackend] | None = None,
    ) -> None:
        """Initialize credential manager with configurable backend preference.

        Args:
            project_id: Infisical project ID (can be set via env var)
            environment: Infisical environment (default: "dev")
            client_id: Infisical client ID for universal auth
            client_secret: Infisical client secret for universal auth
            backend_preference: Ordered list of backends to try
                (default: env -> infisical -> keychain)
        """
        self.project_id = project_id or os.getenv(self.INFISICAL_PROJECT_ID)
        self.environment = environment or os.getenv(self.INFISICAL_ENVIRONMENT, "dev")
        self.client_id = client_id or os.getenv(self.INFISICAL_CLIENT_ID)
        self.client_secret = client_secret or os.getenv(self.INFISICAL_CLIENT_SECRET)

        self.backend_preference = backend_preference or [
            CredentialBackend.ENVIRONMENT,
            CredentialBackend.INFISICAL,
            CredentialBackend.KEYCHAIN,
        ]

        self._infisical_client = None
        self._authenticated = False

    def _get_infisical_client(self) -> Any | None:
        """Initialize and return Infisical client."""
        if self._infisical_client is None:
            try:
                from infisicalsdk import InfisicalSDKClient

                # Initialize client with optional host override
                host = os.getenv("INFISICAL_HOST", "https://app.infisical.com")
                self._infisical_client = InfisicalSDKClient(host=host)

            except ImportError:
                logger.warning("Infisical SDK not available, falling back to other backends")
                return None

        return self._infisical_client

    def _authenticate_infisical(self) -> bool:
        """Authenticate with Infisical if credentials are available."""
        if self._authenticated:
            return True

        if not self.client_id or not self.client_secret:
            logger.debug("Infisical credentials not configured, skipping")
            return False

        client = self._get_infisical_client()
        if not client:
            return False

        try:
            client.auth.universal_auth.login(
                client_id=self.client_id, client_secret=self.client_secret
            )
            self._authenticated = True
            logger.debug("Successfully authenticated with Infisical")
            return True

        except Exception as e:
            logger.warning(f"Infisical authentication failed: {e}")
            return False

    def _get_from_environment(self, key: str) -> str | None:
        """Get credential from environment variables."""
        return os.getenv(key)

    def _get_from_infisical(self, key: str) -> str | None:
        """Get credential from Infisical."""
        if not self.project_id:
            logger.debug("Infisical project ID not configured")
            return None

        if not self._authenticate_infisical():
            return None

        client = self._get_infisical_client()
        if not client:
            return None

        try:
            secret = client.secrets.get_secret_by_name(
                secret_name=key,
                project_id=self.project_id,
                environment_slug=self.environment,
                secret_path="/",
            )
            return secret.secret_value

        except Exception as e:
            logger.debug(f"Failed to get {key} from Infisical: {e}")
            return None

    def _get_from_keychain(self, key: str) -> str | None:
        """Get credential from system keychain (backward compatibility)."""
        try:
            import keyring

            return keyring.get_password(self.SERVICE_NAME, key)
        except Exception as e:
            logger.debug(f"Keychain access failed for {key}: {e}")
            return None

    def _set_to_infisical(self, key: str, value: str) -> bool:
        """Store credential in Infisical."""
        if not self.project_id:
            logger.debug("Infisical project ID not configured, cannot store secret")
            return False

        if not self._authenticate_infisical():
            return False

        client = self._get_infisical_client()
        if not client:
            return False

        try:
            client.secrets.create_secret(
                secret_name=key,
                secret_value=value,
                project_id=self.project_id,
                environment_slug=self.environment,
                secret_path="/",
            )
            logger.debug(f"Successfully stored {key} in Infisical")
            return True

        except Exception as e:
            # Try to update if it already exists
            try:
                client.secrets.update_secret(
                    secret_name=key,
                    secret_value=value,
                    project_id=self.project_id,
                    environment_slug=self.environment,
                    secret_path="/",
                )
                logger.debug(f"Successfully updated {key} in Infisical")
                return True
            except Exception as update_e:
                logger.warning(f"Failed to store/update {key} in Infisical: {e}, {update_e}")
                return False

    def _set_to_keychain(self, key: str, value: str) -> bool:
        """Store credential in system keychain (backward compatibility)."""
        try:
            import keyring

            keyring.set_password(self.SERVICE_NAME, key, value)
            return True
        except Exception as e:
            logger.warning(f"Failed to store {key} in keychain: {e}")
            return False

    def _delete_from_infisical(self, key: str) -> bool:
        """Delete credential from Infisical."""
        if not self.project_id:
            return False

        if not self._authenticate_infisical():
            return False

        client = self._get_infisical_client()
        if not client:
            return False

        try:
            client.secrets.delete_secret_by_name(
                secret_name=key,
                project_id=self.project_id,
                environment_slug=self.environment,
                secret_path="/",
            )
            return True
        except Exception as e:
            logger.debug(f"Failed to delete {key} from Infisical: {e}")
            return False

    def _delete_from_keychain(self, key: str) -> bool:
        """Delete credential from system keychain."""
        try:
            import keyring

            keyring.delete_password(self.SERVICE_NAME, key)
            return True
        except Exception as e:
            logger.debug(f"Failed to delete {key} from keychain: {e}")
            return False

    def get(self, key: str) -> str | None:
        """
        Retrieve a secret using the configured backend preference.

        Args:
            key: The credential key to retrieve

        Returns:
            The credential value or None if not found
        """
        for backend in self.backend_preference:
            try:
                if backend == CredentialBackend.ENVIRONMENT:
                    value = self._get_from_environment(key)
                elif backend == CredentialBackend.INFISICAL:
                    value = self._get_from_infisical(key)
                elif backend == CredentialBackend.KEYCHAIN:
                    value = self._get_from_keychain(key)
                else:
                    continue

                if value:
                    logger.debug(f"Retrieved {key} from {backend.value}")
                    return value

            except Exception as e:
                logger.debug(f"Failed to get {key} from {backend.value}: {e}")
                continue

        logger.debug(f"Could not retrieve {key} from any backend")
        return None

    def set(self, key: str, value: str) -> None:
        """Store a secret using available backends."""
        stored = False

        # Try to store in Infisical first if configured
        if (CredentialBackend.INFISICAL in self.backend_preference
                and self.project_id and self._set_to_infisical(key, value)):
            stored = True

        # Also store in keychain for backward compatibility
        if (CredentialBackend.KEYCHAIN in self.backend_preference
                and self._set_to_keychain(key, value)):
            stored = True

        if not stored:
            raise RuntimeError(f"Failed to store credential {key} in any backend")

    def delete(self, key: str) -> None:
        """Remove a secret from all backends."""
        # Delete from Infisical
        if self.project_id:
            self._delete_from_infisical(key)

        # Delete from keychain
        self._delete_from_keychain(key)

    @staticmethod
    def generate_api_key() -> str:
        """Generate a secure API key for SSE authentication."""
        return secrets.token_urlsafe(32)

    async def discover_loxone_servers(self, timeout: float = 5.0) -> list[dict[str, str]]:
        """Discover Loxone Miniservers on the local network using multiple methods."""
        # Import original implementation for compatibility
        from .credentials import LoxoneSecrets

        return await LoxoneSecrets.discover_loxone_servers(timeout)

    async def _test_connection(self, host: str, username: str, password: str) -> dict[str, Any]:
        """Test connection to Loxone Miniserver."""
        # Import original implementation for compatibility
        from .credentials import LoxoneSecrets

        return await LoxoneSecrets._test_connection(host, username, password)

    def setup(self) -> None:
        """Interactive setup wizard for configuring Loxone credentials with Infisical support."""
        print("🔐 Enhanced Loxone MCP Server Setup (with Infisical)")
        print("=" * 55)

        # Check if Infisical configuration is available
        infisical_configured = bool(self.project_id and self.client_id and self.client_secret)

        if infisical_configured:
            print("✅ Infisical configuration detected:")
            print(f"   Project: {self.project_id}")
            print(f"   Environment: {self.environment}")
            print("   Authentication: Universal Auth")
        else:
            print("Info: Infisical not configured - will use keychain storage")
            print("   To enable Infisical, set these environment variables:")
            print("   - INFISICAL_PROJECT_ID")
            print("   - INFISICAL_CLIENT_ID")
            print("   - INFISICAL_CLIENT_SECRET")
            print("   - INFISICAL_ENVIRONMENT (optional, defaults to 'dev')")

        # Try to discover Loxone servers first
        discovered_servers = asyncio.run(self.discover_loxone_servers())

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
        if infisical_configured:
            print("in Infisical and system keychain.\n")
        else:
            print("in your system keychain.\n")

        # Check for existing credentials
        existing = self.get(self.HOST_KEY) is not None
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
        test_result = asyncio.run(self._test_connection(host, username, password))

        if not test_result["success"]:
            print(f"\n❌ Connection failed: {test_result['error']}")
            retry = input("\nWould you like to try again? [Y/n]: ")
            if retry.lower() != "n":
                self.setup()  # Restart setup
                return
            else:
                sys.exit(1)

        print("\n✅ Successfully connected to Loxone Miniserver!")
        if test_result.get("info"):
            print(f"   Miniserver: {test_result['info'].get('name', 'Unknown')}")
            print(f"   Version: {test_result['info'].get('version', 'Unknown')}")

        # Store credentials
        try:
            self.set(self.HOST_KEY, host)
            self.set(self.USER_KEY, username)
            self.set(self.PASS_KEY, password)

            print("\n✅ Credentials stored successfully!")
            print(f"   Host: {host}")
            print(f"   User: {username}")
            print(f"   Pass: {'*' * len(password)}")

            if infisical_configured:
                print(f"   Stored in: Infisical ({self.environment}) + Keychain")
            else:
                print("   Stored in: System Keychain")

        except Exception as e:
            print(f"\n❌ Error storing credentials: {e}")
            sys.exit(1)

        # Setup SSE API key for web integrations
        print("\n🌐 SSE Server Setup (for web integrations like n8n, Home Assistant)")
        print("=" * 60)

        existing_api_key = self.get(self.SSE_API_KEY)
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
                    api_key = self.generate_api_key()
                    try:
                        self.set(self.SSE_API_KEY, api_key)
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
                        self.set(self.SSE_API_KEY, api_key)
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

        if self.get(self.SSE_API_KEY):
            print("4. Use API key for web integrations (n8n, Home Assistant)")
        else:
            print("4. Configure SSE API key later if needed for web integrations")

        if infisical_configured:
            print("\n🎯 Infisical Integration:")
            print("   • Credentials are now synchronized with Infisical")
            print("   • Team members can access the same credentials")
            print("   • Use different environments (dev/staging/prod) as needed")

    def clear_all(self) -> None:
        """Remove all stored credentials from all backends."""
        keys_to_delete = [self.HOST_KEY, self.USER_KEY, self.PASS_KEY, self.SSE_API_KEY]

        for key in keys_to_delete:
            self.delete(key)

        print("✅ All credentials cleared from all storage backends")

    def validate(self) -> bool:
        """Check if all required credentials are available."""
        required = [self.HOST_KEY, self.USER_KEY, self.PASS_KEY]
        missing = [key for key in required if not self.get(key)]

        if missing:
            print(f"❌ Missing credentials: {', '.join(missing)}")
            print("Run 'uvx --from . loxone-mcp setup' to configure")
            return False

        # Check SSE API key (optional but warn if missing)
        if not self.get(self.SSE_API_KEY):
            print("⚠️  SSE API key not configured - SSE server will generate one automatically")
            print("   For production use, run setup again or set LOXONE_SSE_API_KEY")

        # Show configuration summary
        backend_info = []
        if self.project_id:
            backend_info.append(f"Infisical ({self.environment})")
        if CredentialBackend.KEYCHAIN in self.backend_preference:
            backend_info.append("Keychain")
        if CredentialBackend.ENVIRONMENT in self.backend_preference:
            backend_info.append("Environment")

        print(f"✅ Credentials available from: {', '.join(backend_info)}")
        return True

    def migrate_from_keychain(self) -> None:
        """Migrate existing keychain credentials to Infisical."""
        if not self.project_id:
            print("❌ Infisical project ID not configured. Cannot migrate.")
            print("   Set INFISICAL_PROJECT_ID environment variable")
            return

        if not self._authenticate_infisical():
            print("❌ Failed to authenticate with Infisical. Cannot migrate.")
            return

        print("🔄 Migrating credentials from keychain to Infisical...")

        keys_to_migrate = [self.HOST_KEY, self.USER_KEY, self.PASS_KEY, self.SSE_API_KEY]
        migrated = 0

        for key in keys_to_migrate:
            value = self._get_from_keychain(key)
            if value:
                if self._set_to_infisical(key, value):
                    print(f"   ✅ Migrated {key}")
                    migrated += 1
                else:
                    print(f"   ❌ Failed to migrate {key}")
            else:
                print(f"   ⏭️  {key} not found in keychain")

        print(f"\n📊 Migration complete: {migrated}/{len(keys_to_migrate)} credentials migrated")

        if migrated > 0:
            print("🎯 Credentials are now available in both Infisical and keychain")
            print("   Consider updating your deployment to use Infisical primarily")


# Create a singleton instance for backward compatibility
default_secrets = InfisicalSecrets()

# Backward compatibility aliases
LoxoneSecrets = InfisicalSecrets

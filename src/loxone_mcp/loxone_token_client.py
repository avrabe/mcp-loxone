"""Token-based Loxone HTTP client following official API documentation.

Implements proper JWT token authentication as recommended by Loxone.
Replaces basic auth with secure token-based authentication.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import base64
import hashlib
import hmac
import logging
import secrets
import time
import uuid
from typing import Any
from urllib.parse import quote

import httpx
from cryptography import x509
from cryptography.hazmat.primitives.asymmetric import padding
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

logger = logging.getLogger(__name__)


class LoxoneTokenClient:
    """Token-based Loxone HTTP client using JWT authentication."""

    def __init__(
        self,
        host: str,
        username: str,
        password: str,
        port: int = 80,
        max_reconnect_attempts: int = 5,
        reconnect_delay: float = 5.0,
    ) -> None:
        """
        Initialize Loxone token-based HTTP client.

        Args:
            host: Miniserver IP address or hostname
            username: Loxone username
            password: Loxone password
            port: Port number (default 80)
            max_reconnect_attempts: Maximum reconnection attempts (default 5)
            reconnect_delay: Delay between reconnection attempts in seconds (default 5.0)
        """
        self.host = host
        self.port = port
        self.username = username
        self.password = password
        self.base_url = f"http://{host}:{port}"

        # HTTP client without auth (we'll handle tokens ourselves)
        self.client = httpx.AsyncClient(timeout=30.0)

        # Token management
        self.token: str | None = None
        self.token_valid_until: int | None = None
        self.token_rights: int | None = None
        self.key: str | None = None

        # Encryption (disabled by default due to certificate parsing issues)
        self.public_key: Any | None = None
        self.use_encryption: bool = False  # Can be enabled but requires valid certificate format

        # Cache for structure
        self.structure: dict[str, Any] | None = None

        # Client UUID for token management
        self.client_uuid = str(uuid.uuid4())

        # WebSocket for real-time monitoring
        self.websocket_client: Any | None = None
        self.realtime_monitoring = False

        # Connection state
        self.connected = False
        self.last_successful_command = time.time()

        # Reconnection settings
        self.max_reconnect_attempts = max_reconnect_attempts
        self.reconnect_delay = reconnect_delay
        self.reconnect_task: asyncio.Task | None = None
        self._reconnection_event = asyncio.Event()
        self._reconnection_event.set()  # Initially not reconnecting

    async def connect(self) -> None:
        """Initialize connection and acquire token with automatic retry."""
        attempt = 0
        while attempt < self.max_reconnect_attempts:
            try:
                logger.info(
                    f"Connecting to Loxone at {self.base_url} "
                    f"(attempt {attempt + 1}/{self.max_reconnect_attempts})"
                )

                # Step 1: Check if Miniserver is reachable
                await self._check_reachability()

                # Step 2: Get public key for encryption (if enabled)
                if self.use_encryption:
                    await self._get_public_key()

                # Step 3: Acquire JWT token
                await self._acquire_token()

                # Step 4: Test connection by loading structure
                await self.get_structure_file()

                self.connected = True
                self.last_successful_command = time.time()
                logger.info(
                    "Successfully connected with token authentication"
                    + (" and encryption" if self.use_encryption else "")
                )
                return

            except Exception as e:
                attempt += 1
                if attempt < self.max_reconnect_attempts:
                    logger.warning(
                        f"Connection attempt {attempt} failed: {e}. "
                        f"Retrying in {self.reconnect_delay} seconds..."
                    )
                    await asyncio.sleep(self.reconnect_delay)
                else:
                    logger.error(
                        f"Failed to connect after {self.max_reconnect_attempts} attempts: {e}"
                    )
                    raise

    async def close(self) -> None:
        """Close HTTP client and kill token."""
        logger.info("Closing Loxone client...")

        # Close WebSocket if active
        if self.websocket_client:
            try:
                logger.debug("Closing WebSocket connection...")
                await self.websocket_client.close()
                self.websocket_client = None
                self.realtime_monitoring = False
            except Exception as e:
                logger.warning(f"Failed to close WebSocket: {e}")

        # Kill token with improved error handling
        if self.token:
            try:
                logger.debug("Killing authentication token...")
                await self._kill_token()
                self.token = None
                self.key = None
            except Exception as e:
                logger.warning(f"Failed to kill token: {e}")

        # Close HTTP client
        try:
            await self.client.aclose()
            logger.info("Loxone client closed successfully")
        except Exception as e:
            logger.warning(f"Error closing HTTP client: {e}")
        finally:
            self.connected = False

    async def _check_reachability(self) -> dict[str, Any]:
        """Check if Miniserver is reachable via jdev/cfg/apiKey."""
        try:
            response = await self.client.get(f"{self.base_url}/jdev/cfg/apiKey")
            response.raise_for_status()

            data = response.json()
            config_version = data.get("LL", {}).get("value", "unknown")
            logger.info(f"Miniserver reachable. Config version: {config_version}")

            # Check if it's a local connection
            if "local" in data.get("LL", {}):
                logger.info(f"Connection type: {'local' if data['LL']['local'] else 'remote'}")

            return data

        except Exception as e:
            logger.error(f"Failed to reach Miniserver: {e}")
            raise

    async def _get_public_key(self) -> None:
        """Get RSA public key for command encryption."""
        try:
            logger.debug("Getting RSA public key for encryption...")
            key_response = await self.client.get(f"{self.base_url}/jdev/sys/getPublicKey")
            key_response.raise_for_status()

            # Parse JSON response
            key_data = key_response.json()
            if key_data.get("LL", {}).get("Code") != "200":
                raise ValueError(f"Failed to get public key: {key_data}")

            # Extract certificate from response
            cert_pem = key_data["LL"]["value"]

            # Load certificate and extract public key
            cert = x509.load_pem_x509_certificate(cert_pem.encode("utf-8"))
            self.public_key = cert.public_key()

            logger.debug("Successfully loaded RSA public key from certificate")

        except Exception as e:
            logger.warning(f"Failed to get public key for encryption: {e}")
            logger.info("Disabling encryption due to key retrieval failure")
            self.use_encryption = False

    async def _acquire_token(self) -> None:
        """Acquire JWT token following official authentication flow."""
        try:
            # Step 1: Get key, salt, and hash algorithm
            logger.debug("Getting key, salt and hash algorithm...")
            key_url = f"{self.base_url}/jdev/sys/getkey2/{quote(self.username)}"
            key_response = await self.client.get(key_url)
            key_response.raise_for_status()

            key_data = key_response.json()
            if key_data.get("LL", {}).get("code") != "200":
                raise ValueError(f"Failed to get key: {key_data}")

            key_info = key_data["LL"]["value"]  # Data is nested under 'value'
            key = key_info["key"]
            user_salt = key_info["salt"]
            hash_alg = key_info.get("hashAlg", "SHA1")  # Default to SHA1 for older versions

            logger.debug(f"Retrieved key, salt, hash algorithm: {hash_alg}")

            # Step 2: Hash password with user salt
            hasher = (
                hashlib.sha256()
                if hash_alg.upper() == "SHA256"
                else hashlib.sha1(usedforsecurity=False)
            )

            hasher.update(f"{self.password}:{user_salt}".encode())
            pw_hash = hasher.hexdigest().upper()

            # Step 3: Create HMAC hash with username
            if hash_alg.upper() == "SHA256":
                hmac_hash = hmac.new(
                    bytes.fromhex(key), f"{self.username}:{pw_hash}".encode(), hashlib.sha256
                ).hexdigest()
            else:
                hmac_hash = hmac.new(
                    bytes.fromhex(key), f"{self.username}:{pw_hash}".encode(), hashlib.sha1
                ).hexdigest()

            # Step 4: Request JWT token
            # Permission 4 = App permission (long-lived token)
            permission = 4
            client_info = quote("Python MCP Client")

            logger.debug("Requesting JWT token...")

            # Note: According to docs, this should be encrypted, but let's try unencrypted first
            # for Gen 1 compatibility
            token_url = (
                f"{self.base_url}/jdev/sys/getjwt/{hmac_hash}/"
                f"{quote(self.username)}/{permission}/{self.client_uuid}/{client_info}"
            )

            token_response = await self.client.get(token_url)
            token_response.raise_for_status()

            token_data = token_response.json()
            if token_data.get("LL", {}).get("code") != "200":
                raise ValueError(f"Failed to get token: {token_data}")

            token_info = token_data["LL"]["value"]
            self.token = token_info["token"]  # JWT token is under 'token' key
            self.token_valid_until = token_info.get("validUntil")
            self.token_rights = token_info.get("tokenRights")
            self.key = token_info.get("key")

            logger.info(f"Successfully acquired JWT token. Valid until: {self.token_valid_until}")

            if token_info.get("unsecurePass"):
                logger.warning("⚠️ Weak password detected! Please change your password.")

        except Exception as e:
            logger.error(f"Failed to acquire token: {e}")
            raise

    async def _kill_token(self) -> None:
        """Kill the current token."""
        if not self.token or not self.key:
            return

        try:
            # Use plaintext token for kill request (v11.2+)
            kill_url = f"{self.base_url}/jdev/sys/killtoken/{self.token}/{quote(self.username)}"

            # Set a shorter timeout for token cleanup to avoid blocking shutdown
            async with httpx.AsyncClient(timeout=5.0) as client:
                response = await client.get(kill_url)

            if response.status_code == 200:
                logger.info("Token killed successfully")
            elif response.status_code == 401:
                # Token was already invalid - this is expected during shutdown
                logger.debug("Token was already invalid (expected during shutdown)")
            else:
                logger.warning(f"Failed to kill token: {response.status_code}")

        except (httpx.TimeoutException, httpx.ConnectTimeout):
            logger.warning("Timeout while killing token (server may be shutting down)")
        except httpx.HTTPStatusError as e:
            if e.response.status_code == 401:
                logger.debug("Token kill failed with 401 (token already invalid)")
            else:
                logger.warning(f"HTTP error killing token: {e}")
        except Exception as e:
            logger.warning(f"Error killing token: {e}")

    def _encrypt_command(self, command: str) -> str:
        """
        Encrypt command using AES256 + RSA according to Loxone documentation.

        Args:
            command: Command to encrypt (with authentication already included)

        Returns:
            Encrypted command path for use in HTTP request
        """
        if not self.use_encryption or not self.public_key:
            # Return unencrypted command if encryption disabled
            return command

        try:
            # Step 1: Generate random salt (2 bytes)
            salt = secrets.token_hex(2)

            # Step 2: Prepend salt to command
            plaintext = f"salt/{salt}/{command}"

            # Step 3: Generate AES256 key and IV
            aes_key = secrets.token_bytes(32)  # 256 bits
            aes_iv = secrets.token_bytes(16)  # 128 bits

            # Step 4: AES encrypt the plaintext
            cipher = Cipher(algorithms.AES(aes_key), modes.CBC(aes_iv))
            encryptor = cipher.encryptor()

            # Pad plaintext to multiple of 16 bytes (PKCS7 padding)
            plaintext_bytes = plaintext.encode("utf-8")
            padding_length = 16 - (len(plaintext_bytes) % 16)
            padded_plaintext = plaintext_bytes + bytes([padding_length] * padding_length)

            # Encrypt
            encrypted_data = encryptor.update(padded_plaintext) + encryptor.finalize()
            encrypted_b64 = base64.b64encode(encrypted_data).decode("ascii")

            # Step 5: RSA encrypt the AES key + IV
            session_key_data = f"{aes_key.hex()}:{aes_iv.hex()}"
            encrypted_session_key = self.public_key.encrypt(
                session_key_data.encode("utf-8"), padding.PKCS1v15()
            )
            session_key_b64 = base64.b64encode(encrypted_session_key).decode("ascii")

            # Step 6: Create encrypted command URL
            # Use 'enc' for command-only encryption (vs 'fenc' for full encryption)
            encrypted_command = f"jdev/sys/enc/{quote(encrypted_b64)}"
            encrypted_command_with_key = f"{encrypted_command}?sk={quote(session_key_b64)}"

            logger.debug(f"Encrypted command: {command[:50]}...")
            return encrypted_command_with_key

        except Exception as e:
            logger.warning(f"Command encryption failed: {e}, falling back to plaintext")
            return command

    async def _refresh_token_if_needed(self) -> None:
        """Refresh token if it's close to expiry."""
        if not self.token or not self.key or not self.token_valid_until:
            await self._acquire_token()
            return

        # Refresh if token expires in the next 5 minutes
        import time

        current_time = int(time.time()) - 1230768000  # Loxone epoch starts 2009-01-01

        if current_time + 300 >= self.token_valid_until:
            logger.info("Token expiring soon, refreshing...")

            try:
                # Use plaintext token for refresh (v11.2+)
                refresh_url = (
                    f"{self.base_url}/jdev/sys/refreshjwt/{self.token}/{quote(self.username)}"
                )
                response = await self.client.get(refresh_url)
                response.raise_for_status()

                refresh_data = response.json()
                if refresh_data.get("LL", {}).get("code") == "200":
                    refresh_info = refresh_data["LL"]["value"]
                    self.token = refresh_info["token"]
                    self.token_valid_until = refresh_info.get("validUntil")
                    logger.info("Token refreshed successfully")
                else:
                    logger.warning("Token refresh failed, acquiring new token...")
                    await self._acquire_token()

            except Exception as e:
                logger.warning(f"Token refresh failed: {e}, acquiring new token...")
                await self._acquire_token()

    async def send_command(self, command: str) -> Any:
        """
        Send command using token authentication with automatic reconnection.

        Args:
            command: Command to send (e.g., "jdev/sps/io/uuid/state")

        Returns:
            Command response value
        """
        # Check if we need to reconnect
        if not self.connected:
            logger.info("Not connected, attempting to reconnect...")
            await self._ensure_connection()

        await self._refresh_token_if_needed()

        if not self.token or not self.key:
            raise ValueError("No valid token available")

        try:
            # Add token authentication to command
            separator = "&" if "?" in command else "?"
            auth_params = f"autht={self.token}&user={quote(self.username)}"
            authenticated_command = f"{command}{separator}{auth_params}"

            # Encrypt command if encryption is enabled
            final_command = self._encrypt_command(authenticated_command)

            url = f"{self.base_url}/{final_command}"
            response = await self.client.get(url)
            response.raise_for_status()

            data = response.json()

            # Check for authentication errors
            if data.get("LL", {}).get("code") == "401":
                logger.warning("Authentication failed, refreshing token...")
                await self._acquire_token()
                # Retry once with new token
                return await self.send_command(command)

            # Update last successful command time
            self.last_successful_command = time.time()
            return data.get("LL", {}).get("value")

        except (httpx.NetworkError, httpx.TimeoutException) as e:
            logger.error(f"Network error for command '{command}': {e}")
            self.connected = False
            # Try to reconnect and retry
            await self._ensure_connection()
            return await self.send_command(command)
        except Exception as e:
            logger.error(f"Command '{command}' failed: {e}")
            raise

    async def get_structure_file(self) -> dict[str, Any]:
        """Get the LoxAPP3.json structure file."""
        if self.structure is not None:
            return self.structure

        try:
            await self._refresh_token_if_needed()

            if not self.token or not self.key:
                raise ValueError("No valid token available")

            # Use token authentication for structure file (plaintext for v11.2+)
            url = (
                f"{self.base_url}/data/LoxAPP3.json?autht={self.token}&user={quote(self.username)}"
            )
            response = await self.client.get(url)
            response.raise_for_status()

            self.structure = response.json()
            controls_count = len(self.structure.get("controls", {}))
            logger.info(f"Loaded structure file with {controls_count} controls")

            return self.structure

        except Exception as e:
            logger.error(f"Failed to load structure file: {e}")
            raise

    async def check_token_validity(self) -> bool:
        """Check if current token is still valid."""
        if not self.token or not self.key:
            return False

        try:
            # Use plaintext token for v11.2+
            check_url = f"{self.base_url}/jdev/sys/checktoken/{self.token}/{quote(self.username)}"
            response = await self.client.get(check_url)

            if response.status_code == 200:
                data = response.json()
                return data.get("LL", {}).get("code") == "200"
            else:
                return False

        except Exception:
            return False

    async def start_realtime_monitoring(self, state_callback: Any | None = None) -> None:
        """
        Start real-time WebSocket monitoring for sensor states.

        Args:
            state_callback: Optional callback function for state changes (uuid, value)
        """
        if self.realtime_monitoring:
            logger.warning("Real-time monitoring already active")
            return

        if not self.token or not self.username:
            raise ValueError("Authentication required before starting WebSocket monitoring")

        try:
            # Import here to avoid circular imports
            from .loxone_websocket_client import LoxoneWebSocketClient

            logger.info("Starting real-time WebSocket monitoring...")

            self.websocket_client = LoxoneWebSocketClient(self.host, self.port)

            if state_callback:
                self.websocket_client.add_state_callback(state_callback)

            await self.websocket_client.connect(self.token, self.username)
            self.realtime_monitoring = True

            logger.info("Real-time monitoring started successfully")

        except Exception as e:
            logger.error(f"Failed to start real-time monitoring: {e}")
            if self.websocket_client:
                await self.websocket_client.close()
                self.websocket_client = None
            raise

    async def stop_realtime_monitoring(self) -> None:
        """Stop real-time WebSocket monitoring."""
        if not self.realtime_monitoring or not self.websocket_client:
            return

        logger.info("Stopping real-time monitoring...")

        try:
            await self.websocket_client.close()
        except Exception as e:
            logger.warning(f"Error stopping WebSocket: {e}")
        finally:
            self.websocket_client = None
            self.realtime_monitoring = False
            logger.info("Real-time monitoring stopped")

    async def _ensure_connection(self) -> None:
        """Ensure we have a valid connection, reconnecting if necessary."""
        if not self._reconnection_event.is_set():
            # Wait for ongoing reconnection
            await self._reconnection_event.wait()
            return

        if self.connected:
            # Check if connection is still alive
            try:
                await self._check_reachability()
                return
            except Exception:
                logger.info("Connection check failed, marking as disconnected")
                self.connected = False

        # Reconnect
        self._reconnection_event.clear()
        try:
            await self.connect()
        finally:
            self._reconnection_event.set()

    async def _monitor_connection(self) -> None:
        """Background task to monitor connection health."""
        while True:
            try:
                await asyncio.sleep(60)  # Check every minute

                # Check if we've had recent successful commands
                time_since_last_command = time.time() - self.last_successful_command
                if time_since_last_command > 300:  # 5 minutes
                    # Do a health check
                    logger.debug("Performing connection health check...")
                    await self._check_reachability()
                    self.last_successful_command = time.time()

            except Exception as e:
                logger.warning(f"Connection health check failed: {e}")
                self.connected = False

    def get_realtime_state(self, uuid_str: str) -> Any:
        """
        Get current real-time state for UUID from WebSocket.

        Args:
            uuid_str: UUID of the sensor/device

        Returns:
            Current state value or None if not available
        """
        if not self.websocket_client:
            return None

        return self.websocket_client.get_state(uuid_str)

    def get_all_realtime_states(self) -> dict[str, Any]:
        """Get all current real-time states."""
        if not self.websocket_client:
            return {}

        return self.websocket_client.get_all_states()

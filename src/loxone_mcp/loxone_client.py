"""Simple Loxone WebSocket client for Generation 1 Miniservers."""

import asyncio
import contextlib
import hashlib
import hmac
import json
import logging
from typing import Any

import httpx
from websockets import connect as ws_connect

logger = logging.getLogger(__name__)


class LoxoneError(Exception):
    """Base exception for Loxone errors."""

    pass


class LoxoneAuthError(LoxoneError):
    """Authentication related errors."""

    pass


class LoxoneConnectionError(LoxoneError):
    """Connection related errors."""

    pass


class Loxone:
    """Simple Loxone WebSocket client for Gen 1 Miniservers."""

    def __init__(self, host: str, username: str, password: str, port: int = 80) -> None:
        """
        Initialize Loxone client.

        Args:
            host: Miniserver IP address or hostname
            username: Loxone username
            password: Loxone password
            port: Port number (default 80)
        """
        self.host = host
        self.port = port
        self.username = username
        self.password = password

        # WebSocket connection
        self.ws = None
        self.ws_url = f"ws://{host}:{port}/ws/rfc6455"
        self.http_url = f"http://{host}:{port}"

        # State tracking
        self.connected = False
        self.authenticated = False
        self.structure = None
        self.states = {}

        # Message handling
        self.message_id = 0
        self.pending_messages = {}

        # Keepalive
        self.keepalive_task = None
        self.keepalive_interval = 60  # seconds

    async def connect(self) -> None:
        """Establish WebSocket connection to Loxone."""
        try:
            logger.info(f"Connecting to Loxone at {self.ws_url}")
            self.ws = await ws_connect(self.ws_url)
            self.connected = True
            logger.info("WebSocket connection established")

            # Start message handler
            self._message_task = asyncio.create_task(self._message_handler())

        except Exception as e:
            raise LoxoneConnectionError(f"Failed to connect: {e}") from e

    async def authenticate(self) -> bool:
        """Authenticate with the Loxone Miniserver."""
        if not self.connected:
            raise LoxoneConnectionError("Not connected")

        try:
            # Get key for authentication
            response = await self._send_command("jdev/sys/getkey")
            logger.debug(f"getkey response: {response}")

            if response.get("LL", {}).get("control") != "jdev/sys/getkey":
                logger.error(f"Unexpected getkey response structure: {response}")
                raise LoxoneAuthError("Invalid response to getkey")

            key = response["LL"]["value"]
            logger.debug(f"Key (hex): {key}")
            logger.debug(f"Key length: {len(key)} chars, {len(key) // 2} bytes")

            # The key is double-encoded: hex string representing ASCII hex characters
            # First decode from hex to ASCII
            key_ascii = bytes.fromhex(key).decode("ascii")
            logger.debug(f"Key (ASCII): {key_ascii}")

            # Then decode the ASCII hex string to bytes
            key_bytes = bytes.fromhex(key_ascii)
            logger.debug(f"Final key length: {len(key_bytes)} bytes")

            logger.debug(f"Password length: {len(self.password)} chars")
            preview = f"{self.password[:8]}..." if len(self.password) > 8 else "(short)"
            logger.debug(f"Password preview: {preview}")

            # Check if password might be a token or special format
            if len(self.password) > 40:
                logger.info(
                    f"Long password detected ({len(self.password)} chars) - "
                    "might be a token or special format"
                )

            # Wait a moment after getkey
            await asyncio.sleep(0.1)

            # Try authentication with SHA1 hashed password
            # SHA1 is required by Loxone authentication protocol
            password_hash = hashlib.sha1(self.password.encode("utf-8")).hexdigest()  # nosec B324
            auth_string = f"{self.username}:{password_hash}"
            logger.debug(f"Auth string: {self.username}:{password_hash[:8]}...")

            auth_hash = hmac.new(key_bytes, auth_string.encode("utf-8"), hashlib.sha1).hexdigest()  # nosec B324

            logger.debug(f"Auth hash: {auth_hash}")

            # Try authentication
            auth_response = await self._send_command(f"authenticate/{auth_hash}")
            logger.debug(f"Auth response: {auth_response}")

            # Check authentication response
            if isinstance(auth_response, dict):
                ll_data = auth_response.get("LL", {})
                auth_value = ll_data.get("value")
                auth_code = ll_data.get("Code")

                logger.debug(
                    f"Auth code: {auth_code}, Auth value: {auth_value}, Type: {type(auth_value)}"
                )

                # Check various success indicators
                # Some versions return a string token, others return a dict
                if auth_code == "200":
                    # Success! The value might be a session token
                    self.authenticated = True
                    logger.info(f"Successfully authenticated! Token/Value: {auth_value}")

                    # Start keepalive
                    self.keepalive_task = asyncio.create_task(self._keepalive_loop())

                    return True
                else:
                    logger.error(f"Authentication failed with code {auth_code}: {auth_value}")
                    raise LoxoneAuthError(f"Authentication failed: {auth_value}")
            else:
                logger.error(f"Unexpected auth response type: {type(auth_response)}")
                raise LoxoneAuthError("Authentication failed")

        except Exception as e:
            raise LoxoneAuthError(f"Authentication error: {e}") from e

    async def start(self) -> None:
        """Start the client (authenticate and enable updates)."""
        if not self.connected:
            await self.connect()

        if not self.authenticated:
            await self.authenticate()

        # Enable status updates
        await self._send_command("jdev/sps/enablebinstatusupdate")
        logger.info("Binary status updates enabled")

    async def stop(self) -> None:
        """Stop the client gracefully."""
        if self.keepalive_task:
            self.keepalive_task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await self.keepalive_task

        self.authenticated = False
        self.connected = False

    async def close(self) -> None:
        """Close the WebSocket connection."""
        if self.ws:
            await self.ws.close()
            self.ws = None

    async def get_structure_file(self) -> dict[str, Any]:
        """Get the LoxAPP3.json structure file."""
        if self.structure is None:
            async with httpx.AsyncClient() as client:
                response = await client.get(
                    f"{self.http_url}/data/LoxAPP3.json", auth=(self.username, self.password)
                )
                response.raise_for_status()
                self.structure = response.json()

        return self.structure

    async def send_command(self, command: str) -> Any:
        """
        Send a command to Loxone.

        Args:
            command: Command string (e.g., "jdev/sps/io/{uuid}/On")

        Returns:
            Response from Loxone
        """
        if not self.authenticated:
            raise LoxoneError("Not authenticated")

        return await self._send_command(command)

    async def get_state(self, uuid: str) -> Any:
        """Get current state of a control."""
        # For Gen 1, we get states from the structure file
        # In a real implementation, this would track live updates
        return self.states.get(uuid, None)

    async def _send_command(self, command: str) -> dict[str, Any]:
        """Send command and wait for response."""
        if not self.ws:
            raise LoxoneConnectionError("WebSocket not connected")

        # Generate message ID
        msg_id = self.message_id
        self.message_id += 1

        # Create future for response
        future = asyncio.Future()
        self.pending_messages[msg_id] = future

        # Send command
        # For system commands, don't add the jdev/sps/io prefix
        if command.startswith(("jdev/", "authenticate/")) or command == "keepalive":
            message = command
        else:
            message = f"jdev/sps/io/{msg_id}/{command}"

        await self.ws.send(message)
        logger.debug(f"Sent: {message}")

        # Wait for response (with timeout)
        try:
            response = await asyncio.wait_for(future, timeout=10.0)
            return response
        except TimeoutError:
            self.pending_messages.pop(msg_id, None)
            raise LoxoneError(f"Command timeout: {command}") from None

    async def _message_handler(self) -> None:
        """Handle incoming WebSocket messages."""
        try:
            async for message in self.ws:
                try:
                    # Parse message
                    if isinstance(message, bytes):
                        # Binary message - handle status updates
                        await self._handle_binary_message(message)
                    else:
                        # Text message - handle responses
                        data = json.loads(message)
                        await self._handle_text_message(data)

                except json.JSONDecodeError:
                    logger.error(f"Failed to parse message: {message}")
                except Exception as e:
                    logger.error(f"Error handling message: {e}")

        except Exception as e:
            logger.error(f"Message handler error: {e}")
            self.connected = False

    async def _handle_text_message(self, data: dict[str, Any]) -> None:
        """Handle text message responses."""
        logger.debug(f"Handling text message: {data}")

        # Check if this is a response to a pending message
        if "LL" in data:
            data["LL"].get("control", "")

            # Direct response - find the first waiting future
            for msg_id, future in list(self.pending_messages.items()):
                if not future.done():
                    future.set_result(data)
                    self.pending_messages.pop(msg_id)
                    break

    async def _handle_binary_message(self, data: bytes) -> None:
        """Handle binary status update messages."""
        # Binary messages contain state updates
        # For Gen 1, these are in a specific binary format
        # This is a simplified implementation
        logger.debug(f"Received binary message: {len(data)} bytes")

    async def _keepalive_loop(self) -> None:
        """Send keepalive messages to maintain connection."""
        while self.authenticated:
            try:
                await asyncio.sleep(self.keepalive_interval)
                await self._send_command("keepalive")
                logger.debug("Keepalive sent")
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Keepalive error: {e}")
                break

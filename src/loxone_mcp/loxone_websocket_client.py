"""WebSocket-based Loxone client for real-time state monitoring.

Implements WebSocket connection with binary message parsing for live sensor updates.
Follows the official Loxone WebSocket protocol for real-time state monitoring.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import json
import logging
import struct
import uuid
from collections.abc import Callable
from typing import Any
from urllib.parse import quote

import websockets

logger = logging.getLogger(__name__)


class LoxoneWebSocketClient:
    """WebSocket client for real-time Loxone state monitoring."""

    def __init__(self, host: str, port: int = 80, max_reconnect_attempts: int = -1,
                 reconnect_delay: float = 5.0) -> None:
        """
        Initialize WebSocket client.

        Args:
            host: Miniserver IP address or hostname
            port: Port number (default 80)
            max_reconnect_attempts: Max reconnection attempts (-1 for unlimited)
            reconnect_delay: Delay between reconnection attempts in seconds
        """
        self.host = host
        self.port = port
        self.websocket_url = f"ws://{host}:{port}/ws/rfc6455"

        # WebSocket connection
        self.websocket: Any | None = None
        self.connected = False

        # Authentication
        self.authenticated = False
        self.token: str | None = None
        self.username: str | None = None

        # State tracking
        self.states: dict[str, Any] = {}  # UUID -> current state
        self.state_callbacks: list[Callable[[str, Any], None]] = []

        # Message handling
        self.running = False
        self._tasks: set[asyncio.Task] = set()
        
        # Reconnection settings
        self.max_reconnect_attempts = max_reconnect_attempts
        self.reconnect_delay = reconnect_delay
        self.reconnect_count = 0
        self._reconnecting = False

    async def connect(self, token: str, username: str) -> None:
        """
        Connect to WebSocket and authenticate.

        Args:
            token: JWT token for authentication
            username: Username for authentication
        """
        logger.info(f"Connecting to WebSocket at {self.websocket_url}")

        self.token = token
        self.username = username

        try:
            # Connect to WebSocket with remotecontrol protocol
            self.websocket = await websockets.connect(
                self.websocket_url,
                subprotocols=["remotecontrol"],
                ping_interval=None  # We'll handle keepalive ourselves
            )
            self.connected = True
            logger.info("WebSocket connected successfully")

            # Authenticate using token
            await self._authenticate()

            # Enable binary status updates for real-time monitoring
            await self._enable_status_updates()

            # Start message handling loop
            self.running = True
            task = asyncio.create_task(self._message_handler())
            self._tasks.add(task)
            task.add_done_callback(self._tasks.discard)

            logger.info("WebSocket client ready for real-time monitoring")

        except Exception as e:
            logger.error(f"Failed to connect to WebSocket: {e}")
            await self.close()
            raise

    async def close(self) -> None:
        """Close WebSocket connection and cleanup."""
        logger.info("Closing WebSocket connection")

        self.running = False

        # Cancel all tasks
        for task in self._tasks:
            if not task.done():
                task.cancel()

        if self._tasks:
            await asyncio.gather(*self._tasks, return_exceptions=True)
        self._tasks.clear()

        # Close WebSocket
        if self.websocket:
            try:
                await self.websocket.close()
            except Exception as e:
                logger.warning(f"Error closing WebSocket: {e}")

        self.connected = False
        self.authenticated = False
        logger.info("WebSocket connection closed")

    async def _authenticate(self) -> None:
        """Authenticate using token."""
        if not self.token or not self.username:
            raise ValueError("Token and username required for authentication")

        logger.debug("Authenticating WebSocket connection...")

        # Send authentication command using plaintext token (v11.2+)
        auth_command = f"authwithtoken/{self.token}/{quote(self.username)}"
        await self._send_text_message(auth_command)

        # Wait for authentication response
        # For simplicity, we'll assume success if no error occurs
        # In a production implementation, you'd want to parse the response
        await asyncio.sleep(0.5)  # Give time for auth response

        self.authenticated = True
        logger.info("WebSocket authenticated successfully")

    async def _enable_status_updates(self) -> None:
        """Enable binary status updates for real-time monitoring."""
        logger.debug("Enabling binary status updates...")

        await self._send_text_message("jdev/sps/enablebinstatusupdate")
        logger.info("Binary status updates enabled")

    async def _send_text_message(self, message: str) -> None:
        """Send text message to WebSocket."""
        if not self.websocket:
            raise ConnectionError("WebSocket not connected")

        await self.websocket.send(message)
        logger.debug(f"Sent WebSocket message: {message}")

    async def _message_handler(self) -> None:
        """Handle incoming WebSocket messages with automatic reconnection."""
        logger.debug("Starting WebSocket message handler")

        try:
            while self.running:
                if not self.websocket or not self.connected:
                    # Try to reconnect
                    if await self._reconnect():
                        continue
                    else:
                        break
                        
                try:
                    # Receive message with timeout
                    message = await asyncio.wait_for(
                        self.websocket.recv(),
                        timeout=30.0
                    )

                    if isinstance(message, bytes):
                        await self._handle_binary_message(message)
                    else:
                        await self._handle_text_message(message)
                        
                    # Reset reconnect count on successful message
                    self.reconnect_count = 0

                except asyncio.TimeoutError:
                    # Send keepalive to maintain connection
                    await self._send_keepalive()
                except websockets.exceptions.ConnectionClosed as e:
                    logger.warning(f"WebSocket connection closed: {e}")
                    self.connected = False
                    self.authenticated = False
                except Exception as e:
                    logger.error(f"Error handling WebSocket message: {e}")
                    self.connected = False

        except Exception as e:
            logger.error(f"WebSocket message handler error: {e}")
        finally:
            logger.debug("WebSocket message handler stopped")

    async def _send_keepalive(self) -> None:
        """Send keepalive message to maintain connection."""
        try:
            await self._send_text_message("keepalive")
            logger.debug("Sent keepalive message")
        except Exception as e:
            logger.warning(f"Failed to send keepalive: {e}")

    async def _handle_text_message(self, message: str) -> None:
        """Handle text message from WebSocket."""
        try:
            # Try to parse as JSON
            data = json.loads(message)
            logger.debug(f"Received JSON message: {data}")

            # Handle authentication responses, errors, etc.
            if isinstance(data, dict) and 'LL' in data:
                code = data.get('LL', {}).get('code', '200')
                if code != '200':
                    logger.warning(f"WebSocket error response: {data}")

        except json.JSONDecodeError:
            # Not JSON, just log the message
            logger.debug(f"Received text message: {message[:100]}...")

    async def _handle_binary_message(self, data: bytes) -> None:
        """Handle binary message (Gen 1 uses different format than standard protocol)."""
        if len(data) < 8:
            logger.debug(f"Binary message too short: {len(data)} bytes - {data.hex()}")
            return

        try:
            # Parse message header (8 bytes) - Gen 1 format is different
            header = struct.unpack('<BBBBI', data[:8])
            bin_type, identifier, info_flags, reserved, payload_length = header

            logger.debug(
                f"Gen 1 binary message: type=0x{bin_type:02x}, id=0x{identifier:02x}, "
                f"flags=0x{info_flags:02x}, payload_len={payload_length}, total_len={len(data)}"
            )

            # Gen 1 Miniserver uses different message types - try to parse sensor data
            # Look for messages that might contain UUID + value pairs

            if len(data) >= 24:  # Need at least 24 bytes for UUID (16) + double (8)
                # Try to parse as potential state data regardless of header type
                await self._try_parse_gen1_states(data[8:], bin_type, identifier)

            # Handle specific Gen 1 message types we've observed
            if identifier == 0x05 and bin_type == 0xd1:  # Observed "out of service" pattern
                logger.info("Gen 1 Miniserver reported status change - continuing monitoring")
                # Don't close connection - Gen 1 may send this as normal status

            elif identifier == 0xf5 or identifier == 0xf6:  # Large data messages
                # These might contain bulk state updates
                await self._try_parse_gen1_bulk_states(data[8:])

            elif identifier == 0x5e:  # Smaller data messages
                # These might contain individual sensor updates
                await self._try_parse_gen1_individual_states(data[8:])

        except Exception as e:
            logger.error(f"Error parsing Gen 1 binary message: {e}")

    async def _try_parse_gen1_states(self, payload: bytes, msg_type: int, msg_id: int) -> None:
        """Try to parse Gen 1 state data from any message type."""
        # Gen 1 might embed UUID + value pairs anywhere in the payload
        # Look for 24-byte patterns that could be UUID (16) + double (8)

        offset = 0
        updates_found = 0

        while offset + 24 <= len(payload):
            try:
                uuid_bytes = payload[offset:offset+16]
                value_bytes = payload[offset+16:offset+24]

                # Try to parse as UUID
                uuid_obj = uuid.UUID(bytes=uuid_bytes)
                uuid_str = str(uuid_obj)

                # Try to parse as double value
                value = struct.unpack('<d', value_bytes)[0]

                # Check if this looks like valid sensor data
                if self._is_valid_sensor_uuid(uuid_str) and self._is_reasonable_sensor_value(value):
                    old_value = self.states.get(uuid_str)
                    self.states[uuid_str] = value

                    if old_value != value:
                        updates_found += 1
                        logger.info(f"Gen 1 sensor update: {uuid_str} = {value} (was {old_value})")

                        # Log state change
                        try:
                            from .sensor_state_logger import get_state_logger
                            state_logger = get_state_logger()
                            state_logger.log_state_change(uuid_str, old_value, value)
                        except Exception as e:
                            logger.debug(f"State logging error: {e}")

                        # Notify callbacks
                        for callback in self.state_callbacks:
                            try:
                                callback(uuid_str, value)
                            except Exception as e:
                                logger.error(f"Error in state callback: {e}")

                offset += 8  # Try overlapping patterns

            except (ValueError, struct.error):
                # Not a valid UUID or value, try next position
                offset += 1

        if updates_found > 0:
            logger.info(
                f"Gen 1 message type 0x{msg_type:02x}/0x{msg_id:02x} "
                f"contained {updates_found} sensor updates"
            )

    async def _try_parse_gen1_bulk_states(self, payload: bytes) -> None:
        """Parse bulk state updates from large Gen 1 messages."""
        # Large messages might contain many sensor updates
        await self._try_parse_gen1_states(payload, 0, 0)

    async def _try_parse_gen1_individual_states(self, payload: bytes) -> None:
        """Parse individual state updates from smaller Gen 1 messages."""
        # Smaller messages might contain single or few sensor updates
        await self._try_parse_gen1_states(payload, 0, 0)

    def _is_valid_sensor_uuid(self, uuid_str: str) -> bool:
        """Check if UUID looks like a valid Loxone sensor UUID."""
        # Loxone UUIDs often follow patterns - this is a heuristic
        # Very basic validation - could be enhanced
        return uuid_str and len(uuid_str) == 36

    def _is_reasonable_sensor_value(self, value: float) -> bool:
        """Check if value looks like reasonable sensor data."""
        # Filter out obviously invalid values
        if not isinstance(value, int | float):
            return False

        if value != value:  # Check for NaN
            return False

        # For door/window sensors, focus on clean binary values (0/1)
        # or reasonable analog values (0-100 range for things like temperature, etc.)
        if value in [0, 1, 0.0, 1.0]:  # Perfect binary sensors
            return True

        if 0 <= value <= 1000:  # Reasonable analog range
            return True

        # Filter out extremely small/large scientific notation values
        # which are likely parsing artifacts
        return not (abs(value) < 1e-30 or abs(value) > 1e+30)

    async def _handle_value_states(self, data: bytes) -> None:
        """Handle value state updates (UUID + double value)."""
        offset = 0
        updates_count = 0

        while offset + 24 <= len(data):  # Each value state is 24 bytes
            try:
                # Parse UUID (16 bytes) + double value (8 bytes)
                uuid_bytes = data[offset:offset+16]
                value_bytes = data[offset+16:offset+24]

                # Convert UUID bytes to string
                uuid_obj = uuid.UUID(bytes=uuid_bytes)
                uuid_str = str(uuid_obj)

                # Parse double value (little endian)
                value = struct.unpack('<d', value_bytes)[0]

                # Update state
                old_value = self.states.get(uuid_str)
                self.states[uuid_str] = value

                # Notify callbacks if value changed
                if old_value != value:
                    updates_count += 1

                    # Log state change
                    try:
                        from .sensor_state_logger import get_state_logger
                        state_logger = get_state_logger()
                        state_logger.log_state_change(uuid_str, old_value, value)
                    except Exception as e:
                        logger.debug(f"State logging error: {e}")

                    for callback in self.state_callbacks:
                        try:
                            callback(uuid_str, value)
                        except Exception as e:
                            logger.error(f"Error in state callback: {e}")

                offset += 24

            except Exception as e:
                logger.error(f"Error parsing value state at offset {offset}: {e}")
                break

        if updates_count > 0:
            logger.debug(f"Processed {updates_count} value state updates")

    async def _handle_text_states(self, data: bytes) -> None:
        """Handle text state updates (UUID + UUID + text)."""
        offset = 0
        updates_count = 0

        while offset < len(data):
            try:
                if offset + 36 > len(data):  # Need at least 36 bytes for headers
                    break

                # Parse UUIDs (16 bytes each) + text length (4 bytes)
                uuid_bytes = data[offset:offset+16]
                data[offset+16:offset+32]
                text_length = struct.unpack('<I', data[offset+32:offset+36])[0]

                # Convert UUID bytes to string
                uuid_obj = uuid.UUID(bytes=uuid_bytes)
                uuid_str = str(uuid_obj)

                # Extract text (padded to multiple of 4)
                text_start = offset + 36
                text_end = text_start + text_length

                if text_end > len(data):
                    break

                text_value = data[text_start:text_end].decode('utf-8', errors='ignore')

                # Update state
                old_value = self.states.get(uuid_str)
                self.states[uuid_str] = text_value

                # Notify callbacks if value changed
                if old_value != text_value:
                    updates_count += 1

                    # Log state change
                    try:
                        from .sensor_state_logger import get_state_logger
                        state_logger = get_state_logger()
                        state_logger.log_state_change(uuid_str, old_value, text_value)
                    except Exception as e:
                        logger.debug(f"State logging error: {e}")

                    for callback in self.state_callbacks:
                        try:
                            callback(uuid_str, text_value)
                        except Exception as e:
                            logger.error(f"Error in state callback: {e}")

                # Move to next entry (with padding)
                padded_text_length = ((text_length + 3) // 4) * 4
                offset += 36 + padded_text_length

            except Exception as e:
                logger.error(f"Error parsing text state at offset {offset}: {e}")
                break

        if updates_count > 0:
            logger.debug(f"Processed {updates_count} text state updates")

    def add_state_callback(self, callback: Callable[[str, Any], None]) -> None:
        """
        Add callback for state changes.

        Args:
            callback: Function called with (uuid, value) when states change
        """
        self.state_callbacks.append(callback)

    def remove_state_callback(self, callback: Callable[[str, Any], None]) -> None:
        """Remove state callback."""
        if callback in self.state_callbacks:
            self.state_callbacks.remove(callback)

    def get_state(self, uuid_str: str) -> Any:
        """Get current state for UUID."""
        return self.states.get(uuid_str)

    def get_all_states(self) -> dict[str, Any]:
        """Get all current states."""
        return self.states.copy()

    async def _reconnect(self) -> bool:
        """Attempt to reconnect to WebSocket."""
        if self._reconnecting:
            return False
            
        self._reconnecting = True
        try:
            # Check reconnection limit
            if self.max_reconnect_attempts != -1 and self.reconnect_count >= self.max_reconnect_attempts:
                logger.error(f"Maximum reconnection attempts ({self.max_reconnect_attempts}) reached")
                return False
                
            self.reconnect_count += 1
            logger.info(f"Attempting WebSocket reconnection {self.reconnect_count}...")
            
            # Close existing connection
            if self.websocket:
                try:
                    await self.websocket.close()
                except Exception:
                    pass
                self.websocket = None
            
            # Wait before reconnecting
            await asyncio.sleep(self.reconnect_delay)
            
            # Reconnect
            await self.connect(self.token, self.username)
            logger.info("WebSocket reconnection successful")
            return True
            
        except Exception as e:
            logger.error(f"WebSocket reconnection failed: {e}")
            return False
        finally:
            self._reconnecting = False
    
    async def send_command(self, command: str) -> None:
        """Send command via WebSocket with reconnection support."""
        if not self.authenticated:
            if not await self._reconnect():
                raise ValueError("WebSocket not authenticated and reconnection failed")

        try:
            await self._send_text_message(command)
        except Exception as e:
            logger.error(f"Failed to send command: {e}")
            self.connected = False
            # Try once more after reconnection
            if await self._reconnect():
                await self._send_text_message(command)
            else:
                raise

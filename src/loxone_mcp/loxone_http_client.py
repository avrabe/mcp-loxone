"""Simple HTTP-based Loxone client for Gen 1 Miniservers."""

import logging
from typing import Any

import httpx

logger = logging.getLogger(__name__)


class LoxoneHTTPClient:
    """HTTP-based Loxone client that uses basic authentication."""

    def __init__(self, host: str, username: str, password: str, port: int = 80) -> None:
        """
        Initialize Loxone HTTP client.

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
        self.base_url = f"http://{host}:{port}"

        # HTTP client with basic auth
        self.client = httpx.AsyncClient(auth=(username, password), timeout=30.0)

        # Cache for structure
        self.structure = None

    async def connect(self) -> None:
        """Initialize connection (load structure)."""
        logger.info(f"Connecting to Loxone at {self.base_url}")

        # Test connection by loading structure
        await self.get_structure_file()
        logger.info("Successfully connected via HTTP")

    async def close(self) -> None:
        """Close HTTP client."""
        await self.client.aclose()

    async def get_structure_file(self) -> dict[str, Any]:
        """Get the LoxAPP3.json structure file."""
        if self.structure is None:
            response = await self.client.get(f"{self.base_url}/data/LoxAPP3.json")
            response.raise_for_status()
            self.structure = response.json()
            logger.info(f"Loaded structure with {len(self.structure.get('controls', {}))} controls")

        return self.structure

    async def send_command(self, command: str) -> Any:
        """
        Send a command to Loxone via HTTP.

        Args:
            command: Command string (e.g., "jdev/sps/io/{uuid}/On")

        Returns:
            Response from Loxone
        """
        # URL encode the command
        url = f"{self.base_url}/{command}"

        logger.debug(f"Sending HTTP command: {command}")

        try:
            response = await self.client.get(url)
            response.raise_for_status()

            # Parse JSON response
            data = response.json()

            # Check for LL response format
            if isinstance(data, dict) and "LL" in data:
                ll_data = data["LL"]
                if ll_data.get("Code") == "200":
                    return ll_data.get("value")
                else:
                    logger.error(f"Command failed: {ll_data}")
                    raise Exception(f"Command failed: {ll_data.get('value', 'Unknown error')}")

            return data

        except httpx.HTTPError as e:
            logger.error(f"HTTP error: {e}")
            raise
        except Exception as e:
            logger.error(f"Command error: {e}")
            raise

    async def get_state(self, uuid: str) -> Any:
        """Get current state of a control via HTTP."""
        # Try to get state from structure first
        if self.structure:
            control = self.structure.get("controls", {}).get(uuid, {})
            states = control.get("states", {})

            # For simple controls, there might be a direct state
            if "value" in states:
                # Would need to fetch actual value via HTTP
                # For now, return the state UUID
                return states["value"]

        # Try direct state query
        try:
            return await self.send_command(f"jdev/sps/io/{uuid}/state")
        except Exception:
            return None

    # Compatibility methods for the existing server
    async def authenticate(self) -> bool:
        """No authentication needed for HTTP - it uses basic auth."""
        return True

    async def start(self) -> None:
        """Start the client (compatibility method)."""
        await self.connect()

    async def stop(self) -> None:
        """Stop the client (compatibility method)."""
        pass


# Alias for compatibility
Loxone = LoxoneHTTPClient

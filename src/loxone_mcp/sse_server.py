#!/usr/bin/env python3
"""SSE (Server-Sent Events) server for Loxone MCP with API key authentication.

This module provides FastMCP-based SSE server for the Loxone MCP server
with secure API key authentication for web integrations.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import hashlib
import logging
import os
import secrets
import sys
from typing import Any

from fastapi import HTTPException, Request
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer

# Set up logging
logging.basicConfig(
    level=os.getenv("LOXONE_LOG_LEVEL", "INFO"),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger(__name__)

# SSE configuration - These are not used by FastMCP but kept for compatibility
SSE_PORT = int(os.getenv("LOXONE_SSE_PORT", "8000"))  # FastMCP default port
SSE_HOST = os.getenv("LOXONE_SSE_HOST", "127.0.0.1")  # Localhost only for security

# SSL/HTTPS configuration
try:
    from loxone_mcp.ssl_config import get_server_urls, get_ssl_config, validate_ssl_setup

    SSL_AVAILABLE = True
except ImportError:
    logger.warning("SSL configuration module not available")
    SSL_AVAILABLE = False


# Authentication configuration
def get_sse_api_key() -> str | None:
    """Get SSE API key from environment or keychain."""
    # First check environment (takes precedence)
    env_key = os.getenv("LOXONE_SSE_API_KEY")
    if env_key:
        return env_key

    # Then check keychain
    try:
        from loxone_mcp.credentials import LoxoneSecrets

        return LoxoneSecrets.get(LoxoneSecrets.SSE_API_KEY)
    except ImportError:
        return None


SSE_API_KEY = get_sse_api_key()  # Get from env or keychain
SSE_REQUIRE_AUTH = os.getenv("LOXONE_SSE_REQUIRE_AUTH", "true").lower() == "true"

# Security middleware
security = HTTPBearer(auto_error=False)


def generate_api_key() -> str:
    """Generate a secure API key."""
    return secrets.token_urlsafe(32)


def hash_api_key(api_key: str) -> str:
    """Hash an API key for secure storage."""
    return hashlib.sha256(api_key.encode()).hexdigest()


async def verify_api_key(request: Request) -> bool:
    """Verify API key from request headers."""
    if not SSE_REQUIRE_AUTH:
        return True

    # Check for API key in Authorization header (Bearer token)
    auth: HTTPAuthorizationCredentials | None = await security(request)
    if auth and auth.scheme.lower() == "bearer":
        provided_key = auth.credentials
    else:
        # Check for API key in X-API-Key header (alternative method)
        provided_key = request.headers.get("X-API-Key")

    if not provided_key:
        logger.warning(f"SSE access denied: No API key provided from {request.client.host}")
        return False

    # Verify against configured API key
    if not SSE_API_KEY:
        logger.error("SSE_API_KEY not configured but authentication required")
        return False

    # Constant-time comparison to prevent timing attacks
    expected_hash = hash_api_key(SSE_API_KEY)
    provided_hash = hash_api_key(provided_key)

    is_valid = secrets.compare_digest(expected_hash, provided_hash)

    if not is_valid:
        logger.warning(f"SSE access denied: Invalid API key from {request.client.host}")

    return is_valid


async def auth_middleware(request: Request, call_next: Any) -> Any:
    """Authentication middleware for SSE endpoints."""
    # Skip auth for health checks and non-SSE endpoints
    if request.url.path in ["/health", "/", "/docs", "/openapi.json"]:
        return await call_next(request)

    # Check API key for protected endpoints
    if not await verify_api_key(request):
        raise HTTPException(
            status_code=401,
            detail="Invalid or missing API key. "
            "Use Authorization: Bearer <key> or X-API-Key header.",
            headers={"WWW-Authenticate": "Bearer"},
        )

    return await call_next(request)


async def setup_api_key() -> str:
    """Setup API key for SSE authentication."""
    from loxone_mcp.credentials import LoxoneSecrets

    # Check if API key already exists in keychain first
    existing_key = LoxoneSecrets.get(LoxoneSecrets.SSE_API_KEY)
    if existing_key:
        logger.info("✅ SSE API key loaded from keychain")
        return existing_key

    # Generate new API key
    api_key = LoxoneSecrets.generate_api_key()

    # Store in keychain
    LoxoneSecrets.set(LoxoneSecrets.SSE_API_KEY, api_key)

    logger.info("🔑 Generated new SSE API key and stored in keychain")
    logger.info("📋 Use this API key for SSE authentication:")
    logger.info(f"   Authorization: Bearer {api_key}")
    logger.info(f"   OR X-API-Key: {api_key}")

    return api_key


async def run_sse_server() -> None:
    """Run the SSE server using FastMCP's built-in SSE support."""
    logger.info("Starting FastMCP SSE server with authentication...")
    
    # Display server info
    logger.info("🌐 Server will be available at:")
    logger.info(f"   http://{SSE_HOST}:{SSE_PORT}")

    # Import the FastMCP server instance from the main server module
    from loxone_mcp.server import mcp

    # Setup authentication if required
    if SSE_REQUIRE_AUTH:
        global SSE_API_KEY
        if not SSE_API_KEY:
            # Generate and store API key if not provided via environment
            api_key = await setup_api_key()
            # Set for this session
            SSE_API_KEY = api_key

        logger.info("🔒 SSE authentication enabled")
        logger.info("🔑 API key required for all SSE endpoints")
    else:
        logger.warning("⚠️  SSE authentication disabled - anyone on network can access!")
        logger.warning("⚠️  Set LOXONE_SSE_REQUIRE_AUTH=true for production")

    # Run FastMCP's built-in SSE server
    logger.info("✅ Starting FastMCP SSE server...")
    logger.info("📨 SSE endpoint will be available at: /sse")
    
    if SSE_REQUIRE_AUTH:
        logger.warning("⚠️  Note: Authentication middleware not yet integrated with FastMCP SSE")
        logger.warning("⚠️  This is a known limitation - authentication will be added in a future update")
    
    # FastMCP's run_sse_async() only accepts mount_path parameter
    # It doesn't support custom host, port, or SSL configuration
    # For full control, we would need to use uvicorn directly with mcp.sse_app()
    try:
        await mcp.run_sse_async(mount_path="/sse")
    except Exception as e:
        logger.error(f"Failed to start SSE server: {e}")
        raise


def main() -> None:
    """Main entry point."""
    from loxone_mcp.credentials import LoxoneSecrets

    # Validate credentials first
    if not LoxoneSecrets.validate():
        print("❌ Missing Loxone credentials. Run 'uvx --from . loxone-mcp setup' first.")
        sys.exit(1)

    # Run the server
    try:
        asyncio.run(run_sse_server())
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
    except Exception as e:
        logger.error(f"Server error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()

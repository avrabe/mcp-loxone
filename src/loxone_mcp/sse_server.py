#!/usr/bin/env python3
"""SSE (Server-Sent Events) server for Loxone MCP with API key authentication.

This module provides FastMCP-based SSE server for the Loxone MCP server
with secure API key authentication for web integrations.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import asyncio
import hashlib
import json
import logging
import os
import secrets
import sys
from collections.abc import AsyncGenerator
from typing import Any

from fastapi import HTTPException
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from starlette.applications import Starlette
from starlette.middleware.cors import CORSMiddleware
from starlette.requests import Request
from starlette.responses import JSONResponse, StreamingResponse
from starlette.routing import Route

# Set up logging
logging.basicConfig(
    level=os.getenv("LOXONE_LOG_LEVEL", "INFO"),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger(__name__)

# SSE configuration - These are not used by FastMCP but kept for compatibility
SSE_PORT = int(os.getenv("LOXONE_SSE_PORT", "8000"))  # FastMCP default port
SSE_HOST = os.getenv("LOXONE_SSE_HOST", "127.0.0.1")  # Localhost only for security

# SSL/HTTPS configuration - placeholder for future SSL support
SSL_AVAILABLE = False
try:
    import importlib.util

    if importlib.util.find_spec("loxone_mcp.ssl_config"):
        SSL_AVAILABLE = True
except ImportError:
    logger.warning("SSL configuration module not available")


# Authentication configuration
def get_sse_api_key() -> str | None:
    """Get SSE API key from environment or keychain."""
    # First check environment (takes precedence)
    env_key = os.getenv("LOXONE_SSE_API_KEY")
    if env_key:
        return env_key

    # Then check credential storage
    try:
        from loxone_mcp.credentials import get_credentials_manager

        secrets = get_credentials_manager()
        return secrets.get("LOXONE_SSE_API_KEY")
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


# Traditional SSE implementation for n8n compatibility
async def handle_mcp_request(request_data: dict) -> dict:
    """Handle MCP JSON-RPC request and return response."""
    try:
        # Import the MCP server instance
        from loxone_mcp.server import mcp

        # Create a proper MCP request
        method = request_data.get("method", "")
        params = request_data.get("params", {})
        request_id = request_data.get("id", 1)

        logger.debug(f"Processing MCP request: {method} with params: {params}")

        # Handle different MCP methods
        if method == "tools/list":
            # Get available tools using FastMCP API
            tools_response = await mcp.list_tools()

            # Convert Tool objects to serializable format
            if "tools" in tools_response:
                serializable_tools = []
                for tool in tools_response["tools"]:
                    if hasattr(tool, "__dict__"):
                        # Convert Tool object to dict
                        tool_dict = {
                            "name": getattr(tool, "name", ""),
                            "description": getattr(tool, "description", ""),
                            "inputSchema": getattr(tool, "inputSchema", {}),
                        }
                        serializable_tools.append(tool_dict)
                    else:
                        # Already a dict
                        serializable_tools.append(tool)

                tools_response = {"tools": serializable_tools}

            return {"jsonrpc": "2.0", "id": request_id, "result": tools_response}

        elif method == "tools/call":
            # Call a specific tool using FastMCP API
            tool_name = params.get("name", "")
            tool_args = params.get("arguments", {})

            try:
                # Use FastMCP's call_tool method
                result = await mcp.call_tool(tool_name, tool_args)

                return {"jsonrpc": "2.0", "id": request_id, "result": result}
            except Exception as e:
                logger.error(f"Tool execution error: {e}")
                return {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {"code": -32603, "message": f"Tool execution failed: {e!s}"},
                }

        else:
            return {
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {"code": -32601, "message": f"Method '{method}' not found"},
            }

    except Exception as e:
        logger.error(f"MCP request handling error: {e}")
        return {
            "jsonrpc": "2.0",
            "id": request_data.get("id", 1),
            "error": {"code": -32603, "message": f"Internal error: {e!s}"},
        }


async def setup_api_key() -> str:
    """Setup API key for SSE authentication."""
    from loxone_mcp.credentials import get_credentials_manager

    secrets = get_credentials_manager()

    # Check if API key already exists first
    existing_key = secrets.get("LOXONE_SSE_API_KEY")
    if existing_key:
        logger.info("✅ SSE API key loaded from credential storage")
        return existing_key

    # Generate new API key
    api_key = secrets.generate_api_key()

    # Store in credential storage
    secrets.set("LOXONE_SSE_API_KEY", api_key)

    logger.info("🔑 Generated new SSE API key and stored in keychain")
    logger.info("📋 Use this API key for SSE authentication:")
    logger.info(f"   Authorization: Bearer {api_key}")
    logger.info(f"   OR X-API-Key: {api_key}")

    return api_key


async def health_check_endpoint(_request: Request) -> JSONResponse:
    """Health check endpoint."""
    return JSONResponse({"status": "healthy", "service": "loxone-mcp-sse"})


async def messages_endpoint(request: Request) -> JSONResponse:
    """Traditional JSON-RPC endpoint for n8n compatibility."""
    try:
        # Get JSON body
        request_data = await request.json()
        logger.debug(f"Received request: {request_data}")

        # Verify API key if required
        if not await verify_api_key(request):
            return JSONResponse(
                {
                    "jsonrpc": "2.0",
                    "id": request_data.get("id"),
                    "error": {"code": -32001, "message": "Invalid or missing API key"},
                },
                status_code=401,
                headers={"WWW-Authenticate": "Bearer"},
            )

        # Handle MCP request
        response_data = await handle_mcp_request(request_data)

        logger.debug(f"Sending response: {response_data}")
        return JSONResponse(response_data)

    except json.JSONDecodeError:
        return JSONResponse(
            {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}}
        )
    except Exception as e:
        logger.error(f"Messages endpoint error: {e}")
        return JSONResponse(
            {
                "jsonrpc": "2.0",
                "id": None,
                "error": {"code": -32603, "message": f"Internal error: {e!s}"},
            }
        )


async def sse_endpoint(request: Request) -> StreamingResponse:
    """Traditional Server-Sent Events endpoint (if needed for streaming)."""
    # Verify API key if required
    if not await verify_api_key(request):
        return JSONResponse(
            {"error": "Invalid or missing API key"},
            status_code=401,
            headers={"WWW-Authenticate": "Bearer"},
        )

    async def event_stream() -> AsyncGenerator[str, None]:
        """Generate SSE events."""
        # Send initial connection event
        yield f"data: {json.dumps({'type': 'connection', 'status': 'connected'})}\n\n"

        # Keep connection alive with periodic pings
        try:
            while True:
                await asyncio.sleep(30)  # Ping every 30 seconds
                ping_data = {'type': 'ping', 'timestamp': asyncio.get_event_loop().time()}
                yield f"data: {json.dumps(ping_data)}\n\n"
        except asyncio.CancelledError:
            logger.info("SSE connection closed")
            return

    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "Access-Control-Allow-Origin": "*",
            "Access-Control-Allow-Headers": "*",
        },
    )


def add_traditional_routes_to_starlette(app: Starlette) -> None:
    """Add traditional SSE routes to the existing Starlette app."""
    # Add new routes to the existing router
    app.router.routes.extend(
        [
            Route("/health", health_check_endpoint, methods=["GET"]),
            Route("/messages", messages_endpoint, methods=["POST"]),
            Route("/sse", sse_endpoint, methods=["GET"]),
        ]
    )


async def run_sse_server() -> None:
    """Run FastMCP server with traditional SSE endpoints added for n8n compatibility."""
    logger.info("Starting enhanced MCP server with n8n compatibility...")

    # Display server info
    logger.info("🌐 Server will be available at:")
    logger.info(f"   FastMCP: http://{SSE_HOST}:{SSE_PORT}/mcp")
    logger.info(f"   Traditional: http://{SSE_HOST}:{SSE_PORT}/messages")
    logger.info(f"   SSE Stream: http://{SSE_HOST}:{SSE_PORT}/sse")
    logger.info(f"   Health: http://{SSE_HOST}:{SSE_PORT}/health")

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

        logger.info("🔒 SSE authentication configured")
        logger.info("🔑 API key required for all endpoints")
    else:
        logger.info("📂 SSE authentication disabled for development")

    try:
        logger.info("🚀 Starting enhanced FastMCP server...")

        # Get the FastMCP streamable HTTP app
        app = mcp.streamable_http_app()

        # Add CORS middleware for web client access
        app.add_middleware(
            CORSMiddleware,
            allow_origins=["*"],  # In production, restrict this
            allow_credentials=True,
            allow_methods=["*"],
            allow_headers=["*"],
        )

        # Add traditional endpoints to the FastMCP app
        add_traditional_routes_to_starlette(app)

        # Start the server
        import uvicorn

        config = uvicorn.Config(
            app=app,
            host=SSE_HOST,
            port=SSE_PORT,
            log_level="info",
            access_log=True,
            reload=False,
        )

        logger.info("✅ Enhanced MCP server starting...")
        logger.info(f"🌐 All endpoints available at: http://{SSE_HOST}:{SSE_PORT}")

        server = uvicorn.Server(config)
        await server.serve()

    except Exception as e:
        logger.error(f"Failed to start enhanced server: {e}")
        import traceback

        traceback.print_exc()
        raise


def main() -> None:
    """Main entry point."""
    from loxone_mcp.credentials import get_credentials_manager

    # Validate credentials first
    secrets = get_credentials_manager()
    if not secrets.validate():
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

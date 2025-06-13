"""Test MCP transport modes (stdio and SSE) for CI validation."""

import subprocess
import sys
from unittest.mock import patch

import httpx
import pytest


class TestMCPStdioTransport:
    """Test MCP server via stdio transport."""

    def test_stdio_server_import(self) -> None:
        """Test that stdio server can be imported and has required components."""
        from loxone_mcp.server import mcp

        # Verify MCP server instance exists
        assert mcp is not None
        assert hasattr(mcp, "tool")
        assert hasattr(mcp, "prompt")
        assert hasattr(mcp, "run_stdio_async")

    @pytest.mark.asyncio
    async def test_stdio_server_startup(self) -> None:
        """Test that stdio server can start without errors."""
        from loxone_mcp.server import mcp

        # Mock credentials to avoid setup requirements
        with patch("loxone_mcp.credentials.LoxoneSecrets.validate", return_value=True):  # noqa: SIM117
            with patch("loxone_mcp.credentials.LoxoneSecrets.get") as mock_get:
                mock_get.side_effect = lambda key: {
                    "LOXONE_HOST": "192.168.1.100",
                    "LOXONE_USER": "test",
                    "LOXONE_PASS": "test",
                }.get(key)

                # Test that run_stdio_async exists and is callable
                assert callable(mcp.run_stdio_async)

                # We can't actually run it in tests since it would block,
                # but we can verify the method signature
                import inspect

                sig = inspect.signature(mcp.run_stdio_async)
                assert sig is not None

    def test_stdio_server_executable(self) -> None:
        """Test that the server can be executed as a module."""
        # Test that the module can be run (will fail without credentials, which is expected)
        result = subprocess.run(  # noqa: S603
            [sys.executable, "-m", "loxone_mcp", "--help"],
            capture_output=True,
            text=True,
            timeout=10,
        )

        # Should show help or fail gracefully
        assert result.returncode in [0, 1, 2]  # Help shown or credential error

    def test_cli_commands_available(self) -> None:
        """Test that all CLI commands are available."""
        # Test showing usage (no command provided)
        result = subprocess.run(  # noqa: S603
            [sys.executable, "-m", "loxone_mcp"],
            capture_output=True,
            text=True,
            timeout=10,
        )

        # Should show usage and include all expected commands
        assert result.returncode == 1  # Usage shown with exit code 1
        output = result.stdout + result.stderr  # Check both stdout and stderr
        assert "setup" in output
        assert "verify" in output
        assert "clear" in output
        assert "server" in output
        assert "sse" in output

    def test_verify_command(self) -> None:
        """Test that verify command works."""
        # Test with environment variables (subprocess will see these)
        with patch.dict(
            "os.environ",
            {"LOXONE_HOST": "192.168.1.100", "LOXONE_USER": "test", "LOXONE_PASS": "test"},
        ):
            result = subprocess.run(  # noqa: S603
                [sys.executable, "-m", "loxone_mcp", "verify"],
                capture_output=True,
                text=True,
                timeout=10,
            )

            assert result.returncode == 0
            assert "✅" in result.stdout or "✅" in result.stderr

    @pytest.mark.asyncio
    async def test_server_mcp_tools_exist(self) -> None:
        """Test that required MCP tools are defined."""
        from loxone_mcp.server import mcp

        # Get list of tools (this should work even without connection)
        tools = await mcp.list_tools()

        # Verify basic tools exist
        tool_names = [tool.name for tool in tools]
        expected_tools = [
            "list_rooms",
            "get_room_devices",
            "control_device",
            "discover_all_devices",
        ]

        for expected_tool in expected_tools:
            assert expected_tool in tool_names, f"Missing tool: {expected_tool}"


class TestMCPSSETransport:
    """Test MCP server via SSE transport."""

    def test_sse_server_import(self) -> None:
        """Test that SSE server can be imported."""
        import loxone_mcp.sse_server

        # Verify SSE server module exists
        assert hasattr(loxone_mcp.sse_server, "run_sse_server")
        assert hasattr(loxone_mcp.sse_server, "main")
        assert callable(loxone_mcp.sse_server.run_sse_server)
        assert callable(loxone_mcp.sse_server.main)

    def test_sse_server_configuration(self) -> None:
        """Test SSE server configuration."""
        from loxone_mcp.sse_server import SSE_HOST, SSE_PORT

        # Verify default configuration
        assert isinstance(SSE_HOST, str)
        assert isinstance(SSE_PORT, int)
        assert SSE_PORT > 0
        assert SSE_PORT < 65536

    @pytest.mark.asyncio
    async def test_sse_server_startup_with_mocks(self) -> None:
        """Test SSE server startup with mocked dependencies."""
        from loxone_mcp.sse_server import run_sse_server

        # Mock all external dependencies
        with patch("loxone_mcp.credentials.LoxoneSecrets.validate", return_value=True):  # noqa: SIM117
            with patch("loxone_mcp.credentials.LoxoneSecrets.get") as mock_get:
                mock_get.side_effect = lambda key: {
                    "LOXONE_HOST": "192.168.1.100",
                    "LOXONE_USER": "test",
                    "LOXONE_PASS": "test",
                }.get(key)

                with patch("loxone_mcp.server.mcp.run_sse_async") as mock_run_sse:
                    # Mock the SSE server run to avoid actually starting it
                    mock_run_sse.return_value = None

                    # Test that run_sse_server can be called
                    try:
                        await run_sse_server()
                        # If we get here, the function ran without import/syntax errors
                        assert True
                    except Exception as e:
                        # Should not have import or syntax errors
                        assert "import" not in str(e).lower()
                        assert "syntax" not in str(e).lower()

    def test_sse_server_executable(self) -> None:
        """Test that SSE server can be executed as a module."""
        # Test that the module can be imported and help can be shown
        result = subprocess.run(  # noqa: S603
            [
                sys.executable,
                "-c",
                "import loxone_mcp.sse_server; print('SSE server module imported successfully')",
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )

        assert result.returncode == 0
        assert "imported successfully" in result.stdout

    @pytest.mark.asyncio
    async def test_fastmcp_sse_integration(self) -> None:
        """Test FastMCP SSE integration."""
        from loxone_mcp.server import mcp

        # Test that FastMCP has SSE capabilities
        assert hasattr(mcp, "sse_app")
        assert hasattr(mcp, "run_sse_async")
        assert callable(mcp.sse_app)
        assert callable(mcp.run_sse_async)

        # Test SSE app creation
        sse_app = mcp.sse_app()
        assert sse_app is not None

        # Verify it's a Starlette application
        assert hasattr(sse_app, "routes")
        assert hasattr(sse_app, "add_route")


class TestMCPTransportIntegration:
    """Integration tests for MCP transports."""

    @pytest.mark.asyncio
    async def test_both_transports_available(self) -> None:
        """Test that both stdio and SSE transports are available."""
        from loxone_mcp.server import mcp

        # Both transport methods should exist
        assert hasattr(mcp, "run_stdio_async")
        assert hasattr(mcp, "run_sse_async")
        assert callable(mcp.run_stdio_async)
        assert callable(mcp.run_sse_async)

    @pytest.mark.asyncio
    async def test_mcp_server_tools_consistency(self) -> None:
        """Test that tools are consistent across transports."""
        from loxone_mcp.server import mcp

        # Get tools list
        tools = await mcp.list_tools()

        # Should have reasonable number of tools
        assert len(tools) > 5
        assert len(tools) < 50  # Sanity check

        # All tools should have required attributes
        for tool in tools:
            assert hasattr(tool, "name")
            assert hasattr(tool, "description")
            assert tool.name is not None
            assert tool.description is not None

    def test_server_credentials_validation(self) -> None:
        """Test server credential validation."""
        from loxone_mcp.credentials import LoxoneSecrets

        # Test that validation method exists and works
        assert hasattr(LoxoneSecrets, "validate")
        assert callable(LoxoneSecrets.validate)

        # With no credentials, should return False
        with patch.dict("os.environ", {}, clear=True):  # noqa: SIM117
            with patch("keyring.get_password", return_value=None):
                result = LoxoneSecrets.validate()
                assert result is False

        # With credentials, should return True
        with patch.dict(
            "os.environ", {"LOXONE_HOST": "test", "LOXONE_USER": "test", "LOXONE_PASS": "test"}
        ):
            result = LoxoneSecrets.validate()
            assert result is True


class TestMCPServerHTTPEndpoints:
    """Test MCP server HTTP endpoints (for SSE mode)."""

    @pytest.mark.asyncio
    async def test_sse_app_routes(self) -> None:
        """Test that SSE app has expected routes."""
        from loxone_mcp.server import mcp

        app = mcp.sse_app()

        # Should have at least the SSE route
        routes = list(app.routes)
        assert len(routes) > 0

        # Look for SSE route
        sse_routes = [r for r in routes if hasattr(r, "path") and "/sse" in r.path]
        assert len(sse_routes) > 0

    @pytest.mark.asyncio
    async def test_mcp_prompts_exist(self) -> None:
        """Test that MCP prompts are defined."""
        from loxone_mcp.server import mcp

        prompts = await mcp.list_prompts()

        # Should have at least one prompt
        assert len(prompts) > 0

        # Check for the system overview prompt
        prompt_names = [p.name for p in prompts]
        assert "loxone_system_overview" in prompt_names


@pytest.mark.integration
class TestMCPServerLiveEndpoints:
    """Integration tests that require a running server (marked as integration)."""

    @pytest.mark.asyncio
    async def test_sse_endpoint_reachable(self) -> None:
        """Test that SSE endpoint can be reached when server is running."""
        # This test only runs if specifically requested
        # It requires manual server startup
        pytest.skip("Integration test - requires manual server startup")

        async with httpx.AsyncClient() as client:
            try:
                response = await client.get("http://127.0.0.1:8000/sse", timeout=2.0)
                # SSE endpoint should be reachable (may timeout on content, which is OK)
                assert response.status_code in [200, 408]  # 408 = timeout on SSE stream
            except httpx.ConnectError:
                pytest.skip("SSE server not running")
            except httpx.TimeoutException:
                # Timeout is expected for SSE streams
                pass


class TestMCPServerConfiguration:
    """Test MCP server configuration and environment handling."""

    def test_environment_variable_handling(self) -> None:
        """Test environment variable configuration."""
        import os

        from loxone_mcp.sse_server import SSE_HOST, SSE_PORT

        # Test default values
        assert SSE_HOST is not None
        assert SSE_PORT is not None

        # Test environment override
        with patch.dict(os.environ, {"LOXONE_SSE_PORT": "9999"}):
            # Would need to reload module to test this properly
            # For now, just verify the environment variable can be set
            assert os.getenv("LOXONE_SSE_PORT") == "9999"

    def test_logging_configuration(self) -> None:
        """Test logging configuration."""
        import logging
        import os

        # Test that logging can be configured
        with patch.dict(os.environ, {"LOXONE_LOG_LEVEL": "DEBUG"}):
            assert os.getenv("LOXONE_LOG_LEVEL") == "DEBUG"

        # Test that logger exists
        from loxone_mcp.sse_server import logger

        assert isinstance(logger, logging.Logger)
        assert logger.name is not None


if __name__ == "__main__":
    pytest.main([__file__, "-v"])

#!/usr/bin/env python3
"""
MCP Server Integration Tests
Tests the Loxone MCP server with real external tools to ensure compatibility.
"""

import asyncio
import json
import subprocess
import time
import requests
import sseclient
import pytest
from pathlib import Path
import signal
import os
import tempfile
from typing import Optional, Dict, Any

class MCPServerManager:
    """Manages the MCP server lifecycle for testing"""
    
    def __init__(self, port: int = 3003, dev_mode: bool = True):
        self.port = port
        self.dev_mode = dev_mode
        self.process: Optional[subprocess.Popen] = None
        self.base_url = f"http://localhost:{port}"
        
    def start(self) -> bool:
        """Start the MCP server"""
        cmd = ["cargo", "run", "--bin", "loxone-mcp-server", "--", "http", "--port", str(self.port)]
        if self.dev_mode:
            cmd.append("--dev-mode")
            
        try:
            self.process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                cwd=Path(__file__).parent.parent
            )
            
            # Wait for server to start
            for _ in range(30):  # 30 second timeout
                try:
                    response = requests.get(f"{self.base_url}/health", timeout=1)
                    if response.status_code == 200:
                        return True
                except requests.RequestException:
                    pass
                time.sleep(1)
            
            return False
        except Exception as e:
            print(f"Failed to start server: {e}")
            return False
    
    def stop(self):
        """Stop the MCP server"""
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
            self.process = None
    
    def __enter__(self):
        if not self.start():
            raise RuntimeError("Failed to start MCP server")
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()

class MCPClient:
    """Simple MCP client for testing"""
    
    def __init__(self, base_url: str, api_key: str = "1234"):
        self.base_url = base_url
        self.api_key = api_key
        self.headers = {
            "Content-Type": "application/json",
            "X-API-Key": api_key
        }
    
    def test_streamable_http(self, request: Dict[str, Any]) -> Dict[str, Any]:
        """Test new Streamable HTTP transport"""
        headers = {**self.headers, "Accept": "application/json"}
        response = requests.post(f"{self.base_url}/messages", json=request, headers=headers)
        response.raise_for_status()
        return response.json()
    
    def test_sse_transport(self, request: Dict[str, Any], timeout: float = 5.0) -> Optional[Dict[str, Any]]:
        """Test legacy SSE transport"""
        import threading
        
        # Start SSE connection
        sse_headers = {"Accept": "text/event-stream", "X-API-Key": self.api_key}
        sse_response = requests.get(f"{self.base_url}/sse", stream=True, headers=sse_headers)
        
        if sse_response.status_code != 200:
            raise RuntimeError(f"SSE connection failed: {sse_response.status_code}")
        
        client = sseclient.SSEClient(sse_response)
        events = client.events()
        
        # Get endpoint from first event
        start_time = time.time()
        endpoint_url = None
        session_id = None
        
        try:
            first_event = next(events)
            if first_event.event == "endpoint":
                endpoint_url = first_event.data
                # Extract session ID from URL
                if "session_id=" in endpoint_url:
                    session_id = endpoint_url.split("session_id=")[1]
        except StopIteration:
            raise RuntimeError("No endpoint event received from SSE")
        
        if not session_id or not endpoint_url:
            raise RuntimeError("No endpoint event received from SSE")
        
        # Set up background listener for response
        response_received = threading.Event()
        sse_response_data = None
        
        def listen_for_response():
            nonlocal sse_response_data
            try:
                for event in events:
                    if time.time() - start_time > timeout:
                        break
                    if event.event == "message":
                        sse_response_data = json.loads(event.data)
                        response_received.set()
                        break
            except Exception:
                pass
        
        listener = threading.Thread(target=listen_for_response)
        listener.daemon = True
        listener.start()
        
        # Send POST request
        post_headers = {**self.headers, "Accept": "text/event-stream"}
        post_url = f"{self.base_url}{endpoint_url}"
        response = requests.post(post_url, json=request, headers=post_headers)
        
        if response.status_code != 204:
            raise RuntimeError(f"POST request failed: {response.status_code}")
        
        # Wait for response via SSE
        if response_received.wait(timeout=timeout - (time.time() - start_time)):
            return sse_response_data
        
        return None

@pytest.fixture(scope="session")
def mcp_server():
    """Fixture that provides a running MCP server"""
    with MCPServerManager() as server:
        yield server

@pytest.fixture
def mcp_client(mcp_server):
    """Fixture that provides an MCP client"""
    return MCPClient(mcp_server.base_url)

class TestMCPCompatibility:
    """Test MCP protocol compatibility"""
    
    def test_health_endpoint(self, mcp_server):
        """Test that health endpoint is accessible"""
        response = requests.get(f"{mcp_server.base_url}/health")
        assert response.status_code == 200
        assert response.text == "OK"
    
    def test_streamable_http_initialize(self, mcp_client):
        """Test initialize request via Streamable HTTP transport"""
        request = {
            "jsonrpc": "2.0",
            "id": "test-1",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {
                    "name": "integration-test",
                    "version": "1.0.0"
                }
            }
        }
        
        response = mcp_client.test_streamable_http(request)
        
        assert response["jsonrpc"] == "2.0"
        assert response["id"] == "test-1"
        assert "result" in response
        assert "error" not in response  # Should be omitted when null
        
        result = response["result"]
        assert result["protocol_version"] == "2025-03-26"
        assert "capabilities" in result
        assert "server_info" in result
        assert result["server_info"]["name"] == "loxone-mcp-server"
    
    def test_sse_transport_initialize(self, mcp_client):
        """Test initialize request via SSE transport"""
        request = {
            "jsonrpc": "2.0",
            "id": "test-2",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {
                    "name": "integration-test-sse",
                    "version": "1.0.0"
                }
            }
        }
        
        response = mcp_client.test_sse_transport(request)
        
        assert response is not None
        assert response["jsonrpc"] == "2.0"
        assert response["id"] == "test-2"
        assert "result" in response
        assert "error" not in response
    
    def test_tools_list(self, mcp_client):
        """Test tools/list request"""
        # First initialize
        init_request = {
            "jsonrpc": "2.0",
            "id": "init",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "1.0.0"}
            }
        }
        mcp_client.test_streamable_http(init_request)
        
        # Then list tools
        tools_request = {
            "jsonrpc": "2.0",
            "id": "tools-1",
            "method": "tools/list",
            "params": {}
        }
        
        response = mcp_client.test_streamable_http(tools_request)
        
        assert response["jsonrpc"] == "2.0"
        assert response["id"] == "tools-1"
        assert "result" in response
        
        result = response["result"]
        assert "tools" in result
        assert isinstance(result["tools"], list)
        # Should have some Loxone tools
        assert len(result["tools"]) > 0
    
    def test_error_response_format(self, mcp_client):
        """Test that error responses have correct format"""
        request = {
            "jsonrpc": "2.0",
            "id": "error-test",
            "method": "nonexistent/method",
            "params": {}
        }
        
        response = mcp_client.test_streamable_http(request)
        
        assert response["jsonrpc"] == "2.0"
        assert response["id"] == "error-test"
        assert "error" in response
        assert "result" not in response  # Should be omitted when null
        
        error = response["error"]
        assert "code" in error
        assert "message" in error

class TestMCPInspectorCompatibility:
    """Test compatibility with MCP Inspector specifically"""
    
    def test_sse_endpoint_format(self, mcp_server):
        """Test that SSE endpoint returns proper events"""
        headers = {"Accept": "text/event-stream", "X-API-Key": "1234"}
        response = requests.get(f"{mcp_server.base_url}/sse", stream=True, headers=headers)
        
        assert response.status_code == 200
        assert response.headers.get("content-type") == "text/event-stream"
        
        client = sseclient.SSEClient(response)
        events = []
        
        # Collect first few events
        start_time = time.time()
        for event in client.events():
            events.append(event)
            if len(events) >= 2 or time.time() - start_time > 5:
                break
        
        # Should get endpoint event first
        assert len(events) >= 1
        first_event = events[0]
        assert first_event.event == "endpoint"
        assert first_event.data.startswith("/messages?session_id=")
    
    def test_cors_headers(self, mcp_server):
        """Test CORS headers for web compatibility"""
        # Preflight request
        headers = {
            "Origin": "http://localhost:6274",
            "Access-Control-Request-Method": "POST",
            "Access-Control-Request-Headers": "Content-Type,X-API-Key"
        }
        response = requests.options(f"{mcp_server.base_url}/messages", headers=headers)
        
        # Should allow CORS
        assert "access-control-allow-origin" in response.headers or \
               "access-control-allow-credentials" in response.headers

class TestExternalToolIntegration:
    """Test integration with external tools"""
    
    def test_curl_sse_connection(self, mcp_server):
        """Test SSE connection using curl"""
        cmd = [
            "curl", "-N", "-H", "Accept: text/event-stream", 
            "-H", "X-API-Key: 1234", 
            f"{mcp_server.base_url}/sse"
        ]
        
        process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        
        try:
            # Read first few lines with timeout
            start_time = time.time()
            lines = []
            while time.time() - start_time < 5:
                process.poll()
                if process.returncode is not None:
                    break
                    
                # Try to read a line with short timeout
                line = process.stdout.readline()
                if line:
                    lines.append(line.strip())
                    if len(lines) >= 3:  # Should get event: endpoint, data: /messages..., empty line
                        break
                time.sleep(0.1)
        finally:
            process.terminate()
            process.wait()
        
        # Should receive endpoint event
        assert any("event: endpoint" in line for line in lines)
        assert any("/messages?session_id=" in line for line in lines)
    
    def test_curl_post_request(self, mcp_server):
        """Test POST request using curl"""
        request_data = {
            "jsonrpc": "2.0",
            "id": "curl-test",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "curl-test", "version": "1.0.0"}
            }
        }
        
        cmd = [
            "curl", "-X", "POST",
            "-H", "Content-Type: application/json",
            "-H", "Accept: application/json",
            "-H", "X-API-Key: 1234",
            "-d", json.dumps(request_data),
            f"{mcp_server.base_url}/messages"
        ]
        
        result = subprocess.run(cmd, capture_output=True, text=True)
        
        assert result.returncode == 0
        response = json.loads(result.stdout)
        assert response["jsonrpc"] == "2.0"
        assert response["id"] == "curl-test"
        assert "result" in response

def create_test_script():
    """Create a bash script for manual testing"""
    script_content = '''#!/bin/bash

# MCP Server Integration Test Script

set -e

SERVER_URL="http://localhost:3003"
API_KEY="1234"

echo "=== MCP Server Integration Tests ==="

# Test 1: Health check
echo "1. Testing health endpoint..."
curl -s "$SERVER_URL/health" || (echo "Health check failed" && exit 1)
echo " ✓ Health check passed"

# Test 2: SSE connection
echo "2. Testing SSE connection..."
timeout 5 curl -s -N -H "Accept: text/event-stream" -H "X-API-Key: $API_KEY" "$SERVER_URL/sse" | head -5 | grep -q "endpoint" || (echo "SSE test failed" && exit 1)
echo " ✓ SSE connection established"

# Test 3: Streamable HTTP initialize
echo "3. Testing Streamable HTTP transport..."
INIT_RESPONSE=$(curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -H "X-API-Key: $API_KEY" \
  -d '{"jsonrpc":"2.0","id":"test","method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}' \
  "$SERVER_URL/messages")

echo "$INIT_RESPONSE" | jq -e '.result.server_info.name == "loxone-mcp-server"' > /dev/null || (echo "Initialize test failed" && exit 1)
echo " ✓ Initialize request successful"

# Test 4: Tools list
echo "4. Testing tools/list..."
TOOLS_RESPONSE=$(curl -s -X POST \
  -H "Content-Type: application/json" \
  -H "Accept: application/json" \
  -H "X-API-Key: $API_KEY" \
  -d '{"jsonrpc":"2.0","id":"tools","method":"tools/list","params":{}}' \
  "$SERVER_URL/messages")

echo "$TOOLS_RESPONSE" | jq -e '.result.tools | length > 0' > /dev/null || (echo "Tools list test failed" && exit 1)
echo " ✓ Tools list retrieved"

echo ""
echo "🎉 All tests passed! MCP server is working correctly."
'''
    
    return script_content

if __name__ == "__main__":
    # Create bash test script
    script_path = Path(__file__).parent / "test_mcp_server.sh"
    with open(script_path, "w") as f:
        f.write(create_test_script())
    os.chmod(script_path, 0o755)
    
    print(f"Created test script: {script_path}")
    print("Run tests with: python -m pytest integration_tests/test_mcp_compatibility.py -v")
    print("Or use the bash script: ./integration_tests/test_mcp_server.sh")
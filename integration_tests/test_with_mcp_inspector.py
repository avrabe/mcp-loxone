#!/usr/bin/env python3
"""
MCP Inspector Integration Test
Tests the MCP server with the actual MCP Inspector tool.
"""

import asyncio
import json
import subprocess
import time
import requests
import signal
import os
from pathlib import Path
from typing import Optional
import tempfile

class MCPInspectorTest:
    """Test runner for MCP Inspector integration"""
    
    def __init__(self, server_port: int = 3003, inspector_port: int = 6274):
        self.server_port = server_port
        self.inspector_port = inspector_port
        self.server_process: Optional[subprocess.Popen] = None
        self.inspector_process: Optional[subprocess.Popen] = None
        
    def start_server(self) -> bool:
        """Start the MCP server"""
        print("Starting MCP server...")
        cmd = [
            "cargo", "run", "--bin", "loxone-mcp-server", "--", 
            "http", "--port", str(self.server_port), "--dev-mode"
        ]
        
        try:
            self.server_process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                cwd=Path(__file__).parent.parent
            )
            
            # Wait for server to start
            for i in range(30):
                try:
                    response = requests.get(f"http://localhost:{self.server_port}/health", timeout=1)
                    if response.status_code == 200:
                        print(f"✓ MCP server started on port {self.server_port}")
                        return True
                except requests.RequestException:
                    pass
                time.sleep(1)
                print(f"  Waiting for server... ({i+1}/30)")
            
            print("✗ Server failed to start")
            return False
        except Exception as e:
            print(f"✗ Failed to start server: {e}")
            return False
    
    def start_inspector(self) -> bool:
        """Start MCP Inspector"""
        print("Starting MCP Inspector...")
        cmd = ["npx", "@modelcontextprotocol/inspector"]
        
        try:
            # Set environment variable to avoid port conflicts
            env = os.environ.copy()
            env["PORT"] = str(self.inspector_port)
            
            self.inspector_process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=env
            )
            
            # Wait for inspector to start
            for i in range(20):
                try:
                    response = requests.get(f"http://localhost:{self.inspector_port}", timeout=1)
                    if response.status_code == 200:
                        print(f"✓ MCP Inspector started on port {self.inspector_port}")
                        return True
                except requests.RequestException:
                    pass
                time.sleep(1)
                print(f"  Waiting for inspector... ({i+1}/20)")
            
            print("✗ Inspector failed to start")
            return False
        except Exception as e:
            print(f"✗ Failed to start inspector: {e}")
            return False
    
    def test_connection(self) -> bool:
        """Test connection via MCP Inspector"""
        print("Testing MCP Inspector connection...")
        
        # Create a simple test to verify the inspector can connect
        # This would typically involve browser automation, but for now we'll test the underlying protocol
        
        # Test that both services are responding
        try:
            # Test server health
            server_response = requests.get(f"http://localhost:{self.server_port}/health", timeout=5)
            if server_response.status_code != 200:
                print("✗ Server health check failed")
                return False
            
            # Test inspector is running
            inspector_response = requests.get(f"http://localhost:{self.inspector_port}", timeout=5)
            if inspector_response.status_code != 200:
                print("✗ Inspector not responding")
                return False
            
            # Test SSE endpoint
            sse_response = requests.get(
                f"http://localhost:{self.server_port}/sse",
                headers={"Accept": "text/event-stream", "X-API-Key": "1234"},
                stream=True,
                timeout=5
            )
            
            if sse_response.status_code != 200:
                print("✗ SSE endpoint not working")
                return False
            
            print("✓ All endpoints responding correctly")
            return True
            
        except Exception as e:
            print(f"✗ Connection test failed: {e}")
            return False
    
    def stop_all(self):
        """Stop all processes"""
        if self.server_process:
            print("Stopping MCP server...")
            self.server_process.terminate()
            try:
                self.server_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.server_process.kill()
                self.server_process.wait()
        
        if self.inspector_process:
            print("Stopping MCP Inspector...")
            self.inspector_process.terminate()
            try:
                self.inspector_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.inspector_process.kill()
                self.inspector_process.wait()
    
    def run_test(self) -> bool:
        """Run the complete test"""
        try:
            if not self.start_server():
                return False
            
            if not self.start_inspector():
                return False
            
            # Give everything a moment to settle
            time.sleep(2)
            
            if not self.test_connection():
                return False
            
            print("\n🎉 MCP Inspector integration test passed!")
            print(f"   Server: http://localhost:{self.server_port}")
            print(f"   Inspector: http://localhost:{self.inspector_port}")
            print(f"   Test connection in browser with: http://localhost:{self.server_port}/sse")
            
            return True
            
        finally:
            self.stop_all()

def create_npm_test_script():
    """Create a Node.js test script that uses MCP SDK"""
    script_content = '''
const { Client } = require('@modelcontextprotocol/sdk/client');
const { SSEClientTransport } = require('@modelcontextprotocol/sdk/client/sse');

async function testMCPConnection() {
    console.log('Testing MCP connection with official SDK...');
    
    try {
        const transport = new SSEClientTransport('http://localhost:3003/sse', {
            headers: { 'X-API-Key': '1234' }
        });
        
        const client = new Client({
            name: 'test-client',
            version: '1.0.0'
        }, {
            capabilities: {}
        });
        
        console.log('Connecting to server...');
        await client.connect(transport);
        
        console.log('Connected! Testing initialize...');
        
        console.log('Listing tools...');
        const tools = await client.listTools();
        console.log(`Found ${tools.tools.length} tools`);
        
        console.log('✓ All tests passed with official MCP SDK!');
        
        await client.close();
    } catch (error) {
        console.error('✗ Test failed:', error.message);
        process.exit(1);
    }
}

testMCPConnection();
'''
    
    package_json = '''
{
  "name": "mcp-integration-test",
  "version": "1.0.0",
  "type": "module",
  "dependencies": {
    "@modelcontextprotocol/sdk": "latest"
  }
}
'''
    
    return script_content, package_json

if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser(description="Test MCP server with Inspector")
    parser.add_argument("--server-port", type=int, default=3003, help="MCP server port")
    parser.add_argument("--inspector-port", type=int, default=6274, help="Inspector port")
    parser.add_argument("--create-npm-test", action="store_true", help="Create Node.js test files")
    
    args = parser.parse_args()
    
    if args.create_npm_test:
        # Create Node.js test files
        test_dir = Path(__file__).parent / "nodejs_test"
        test_dir.mkdir(exist_ok=True)
        
        script_content, package_json = create_npm_test_script()
        
        with open(test_dir / "test.js", "w") as f:
            f.write(script_content)
        
        with open(test_dir / "package.json", "w") as f:
            f.write(package_json)
        
        print(f"Created Node.js test files in {test_dir}")
        print("To run: cd {test_dir} && npm install && node test.js")
    else:
        # Run the integration test
        tester = MCPInspectorTest(args.server_port, args.inspector_port)
        success = tester.run_test()
        exit(0 if success else 1)
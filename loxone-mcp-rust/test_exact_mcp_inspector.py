#!/usr/bin/env python3
"""
Test that exactly mimics MCP Inspector behavior
"""

import requests
import sseclient
import json
import threading
import time

def test_exact_mcp_inspector():
    """Test with exact same headers and behavior as MCP Inspector"""
    
    # Step 1: Connect to SSE with exact MCP Inspector headers
    sse_headers = {
        "Accept": "text/event-stream",
        "X-API-Key": "1234",
        "Accept-Language": "*",
        "Sec-Fetch-Mode": "cors",
        "User-Agent": "node",
        "Pragma": "no-cache",
        "Cache-Control": "no-cache",
        "Accept-Encoding": "gzip, deflate",
        "Connection": "keep-alive"
    }
    
    print("Connecting to SSE with MCP Inspector headers...")
    sse_response = requests.get("http://localhost:3001/sse", stream=True, headers=sse_headers)
    print(f"SSE Status: {sse_response.status_code}")
    
    if sse_response.status_code != 200:
        print(f"SSE connection failed: {sse_response.text}")
        return False
    
    # Step 2: Parse SSE events
    client = sseclient.SSEClient(sse_response)
    events = client.events()
    
    # Get endpoint event
    first_event = next(events)
    print(f"First event: type='{first_event.event}', data='{first_event.data}'")
    
    if first_event.event != "endpoint":
        print("ERROR: Expected 'endpoint' event")
        return False
    
    endpoint_url = first_event.data
    session_id = endpoint_url.split("session_id=")[1] if "session_id=" in endpoint_url else None
    print(f"Endpoint URL: {endpoint_url}")
    print(f"Session ID: {session_id}")
    
    # Step 3: Send initialize with exact MCP Inspector format
    init_request = {
        "jsonrpc": "2.0",
        "id": 0,  # MCP Inspector uses 0 as first ID
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {
                "roots": {"listChanged": True},
                "sampling": {}
            },
            "clientInfo": {
                "name": "mcp-inspector",
                "version": "0.14.0"
            }
        }
    }
    
    # Listen for response in background
    response_received = threading.Event()
    responses = []
    
    def listen_for_responses():
        try:
            for event in events:
                print(f"SSE Event: type='{event.event}', data='{event.data[:100]}...'")
                if event.event == "message":
                    try:
                        response = json.loads(event.data)
                        responses.append(response)
                        response_received.set()
                        print("✅ Response received via SSE")
                        
                        # Validate response format
                        if response.get("jsonrpc") == "2.0" and response.get("id") == 0:
                            result = response.get("result", {})
                            if "protocolVersion" in result and "serverInfo" in result:
                                print("✅ Response format is correct")
                                return True
                            else:
                                print("❌ Response missing required fields")
                        else:
                            print("❌ Invalid JSON-RPC response")
                        
                    except json.JSONDecodeError as e:
                        print(f"❌ JSON parsing error: {e}")
                elif event.event == "ping":
                    print("📡 Received ping")
        except Exception as e:
            print(f"❌ SSE listener error: {e}")
        return False
    
    listener = threading.Thread(target=listen_for_responses)
    listener.daemon = True
    listener.start()
    
    # Send POST with exact MCP Inspector headers
    post_headers = {
        "Content-Type": "application/json",
        "Accept": "text/event-stream",  # Important: MCP Inspector uses SSE accept
        "X-API-Key": "1234",
        "User-Agent": "node",
        "Accept-Language": "*",
        "Sec-Fetch-Mode": "cors",
        "Connection": "keep-alive"
    }
    
    post_url = f"http://localhost:3001{endpoint_url}"
    print(f"Sending POST to: {post_url}")
    print(f"Request: {json.dumps(init_request, indent=2)}")
    
    try:
        post_response = requests.post(post_url, json=init_request, headers=post_headers)
        print(f"POST Status: {post_response.status_code}")
        print(f"POST Headers: {dict(post_response.headers)}")
        print(f"POST Body: '{post_response.text}'")
        
        if post_response.status_code != 204:
            print(f"❌ Unexpected POST status: {post_response.status_code}")
            return False
            
    except Exception as e:
        print(f"❌ POST request failed: {e}")
        return False
    
    # Wait for response
    print("Waiting for SSE response...")
    success = response_received.wait(timeout=5)
    
    if success and responses:
        print("✅ Test passed! MCP Inspector should work.")
        print(f"Response: {json.dumps(responses[0], indent=2)}")
        return True
    else:
        print("❌ Test failed - no response received")
        return False

if __name__ == "__main__":
    success = test_exact_mcp_inspector()
    print(f"\nResult: {'SUCCESS' if success else 'FAILED'}")
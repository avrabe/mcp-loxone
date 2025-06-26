#!/usr/bin/env python3
"""
Debug script to check what MCP Inspector receives
"""

import requests
import sseclient
import json

def debug_mcp_connection():
    # Connect to SSE
    sse_response = requests.get("http://localhost:3001/sse", stream=True, headers={
        "Accept": "text/event-stream",
        "X-API-Key": "1234",
        "User-Agent": "node"
    })
    
    client = sseclient.SSEClient(sse_response)
    events = client.events()
    
    # Get endpoint
    first_event = next(events)
    print(f"First event: {first_event.event} = {first_event.data}")
    endpoint_url = first_event.data
    session_id = endpoint_url.split("session_id=")[1] if "session_id=" in endpoint_url else None
    
    # Send initialize (exactly as MCP Inspector does)
    init_request = {
        "jsonrpc": "2.0",
        "id": 0,
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
    
    print(f"\nSending: {json.dumps(init_request, indent=2)}")
    
    # Send POST first
    post_response = requests.post(
        f"http://localhost:3001{endpoint_url}",
        json=init_request,
        headers={
            "Content-Type": "application/json",
            "Accept": "text/event-stream",
            "X-API-Key": "1234"
        }
    )
    
    print(f"\nPOST status: {post_response.status_code}")
    
    # Now listen for response
    print("\nListening for response...")
    import time
    start_time = time.time()
    for event in events:
        if time.time() - start_time > 5:
            print("Timeout waiting for response")
            break
        if event.event == "message":
            response = json.loads(event.data)
            print(f"\nReceived response: {json.dumps(response, indent=2)}")
            break

if __name__ == "__main__":
    debug_mcp_connection()
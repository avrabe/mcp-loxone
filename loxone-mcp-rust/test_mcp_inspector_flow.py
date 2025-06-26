#!/usr/bin/env python3
"""
Test script to simulate MCP Inspector flow
"""

import requests
import sseclient
import json
import time
import threading

def test_mcp_inspector_flow():
    """Simulate the exact flow MCP Inspector uses"""
    
    # Step 1: Connect to SSE endpoint (like MCP Inspector does)
    print("=== Step 1: Connecting to SSE endpoint ===")
    sse_url = "http://localhost:3001/sse"
    sse_headers = {
        "Accept": "text/event-stream",
        "X-API-Key": "1234",
        "User-Agent": "node",  # MCP Inspector uses node
        "Accept-Language": "*",
        "Sec-Fetch-Mode": "cors",
        "Pragma": "no-cache",
        "Cache-Control": "no-cache"
    }
    
    sse_response = requests.get(sse_url, stream=True, headers=sse_headers)
    print(f"SSE Response Status: {sse_response.status_code}")
    print(f"SSE Response Headers: {dict(sse_response.headers)}")
    
    if sse_response.status_code != 200:
        print(f"Failed to connect to SSE: {sse_response.text}")
        return
    
    client = sseclient.SSEClient(sse_response)
    events = client.events()
    
    # Step 2: Get the endpoint event
    endpoint_url = None
    session_id = None
    
    print("\n=== Step 2: Waiting for endpoint event ===")
    first_event = next(events)
    print(f"Event type: '{first_event.event}', Data: '{first_event.data}'")
    if first_event.event == "endpoint":
        endpoint_url = first_event.data
        # Extract session ID
        if "session_id=" in endpoint_url:
            session_id = endpoint_url.split("session_id=")[1]
        print(f"Got endpoint URL: {endpoint_url}, Session ID: {session_id}")
    
    if not endpoint_url or not session_id:
        print("ERROR: No endpoint event received!")
        return
    
    # Step 3: Start listening for responses in background
    response_received = threading.Event()
    responses = []
    
    def listen_for_responses():
        print("\n=== Background: Listening for SSE responses ===")
        try:
            # We already consumed the first event, so we need to continue from where we left off
            for event in events:
                print(f"SSE Event: type='{event.event}', data='{event.data}'")
                if event.event == "message":
                    try:
                        response = json.loads(event.data)
                        responses.append(response)
                        response_received.set()
                    except json.JSONDecodeError as e:
                        print(f"Failed to parse message: {e}")
                elif event.event == "ping":
                    print("Received ping event")
        except Exception as e:
            print(f"SSE listener error: {e}")
    
    listener = threading.Thread(target=listen_for_responses)
    listener.daemon = True
    listener.start()
    
    # Give the listener a moment to start
    time.sleep(0.1)
    
    # Step 4: Send initialize request
    print(f"\n=== Step 3: Sending initialize request ===")
    init_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {
                "name": "mcp-inspector-test",
                "version": "1.0.0"
            }
        }
    }
    
    # Send to the endpoint URL we received
    post_url = f"http://localhost:3001{endpoint_url}"
    post_headers = {
        "Content-Type": "application/json",
        "Accept": "text/event-stream",  # MCP Inspector uses SSE accept header
        "X-API-Key": "1234",
        "User-Agent": "node",
        "Accept-Language": "*",
        "Sec-Fetch-Mode": "cors"
    }
    
    print(f"POST URL: {post_url}")
    print(f"POST Headers: {post_headers}")
    print(f"POST Body: {json.dumps(init_request, indent=2)}")
    
    try:
        post_response = requests.post(post_url, json=init_request, headers=post_headers)
        print(f"\nPOST Response Status: {post_response.status_code}")
        print(f"POST Response Headers: {dict(post_response.headers)}")
        print(f"POST Response Body: '{post_response.text}'")
    except Exception as e:
        print(f"POST request failed: {e}")
        return
    
    # Step 5: Wait for response via SSE
    print("\n=== Step 4: Waiting for response via SSE ===")
    if response_received.wait(timeout=5):
        print("Received response(s):")
        for resp in responses:
            print(json.dumps(resp, indent=2))
    else:
        print("TIMEOUT: No response received via SSE!")
    
    # Step 6: Send a second request (tools/list)
    print(f"\n=== Step 5: Sending tools/list request ===")
    tools_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }
    
    response_received.clear()
    
    try:
        post_response = requests.post(post_url, json=tools_request, headers=post_headers)
        print(f"POST Response Status: {post_response.status_code}")
    except Exception as e:
        print(f"POST request failed: {e}")
    
    # Wait for response
    if response_received.wait(timeout=5):
        print("Received tools/list response")
    else:
        print("TIMEOUT: No response received for tools/list!")
    
    print("\n=== Test complete ===")
    print(f"Total responses received: {len(responses)}")

if __name__ == "__main__":
    test_mcp_inspector_flow()
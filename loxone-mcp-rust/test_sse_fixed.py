import requests
import sseclient
import json
import time
import threading

def test_sse():
    print("Testing SSE connection...")
    
    # Start SSE connection
    sse_response = requests.get("http://localhost:3002/sse", stream=True, headers={
        "Accept": "text/event-stream",
        "X-API-Key": "1234"
    })
    
    client = sseclient.SSEClient(sse_response)
    events = client.events()
    
    # Get endpoint event
    first_event = next(events)
    print(f"Event type: {first_event.event}, Data: {first_event.data}")
    
    endpoint_url = first_event.data
    session_id = endpoint_url.split("session_id=")[1] if "session_id=" in endpoint_url else None
    print(f"Got endpoint URL: {endpoint_url}, Session ID: {session_id}")
    
    # Keep listening to SSE in background
    response_received = threading.Event()
    sse_response = None
    
    def listen_for_response():
        nonlocal sse_response
        for event in events:
            print(f"Event type: {event.event}, Data: {event.data}")
            if event.event == "message":
                sse_response = json.loads(event.data)
                response_received.set()
                break
    
    listener = threading.Thread(target=listen_for_response)
    listener.daemon = True
    listener.start()
    
    # Send POST request
    init_request = {
        "jsonrpc": "2.0",
        "id": "test-sse",
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "sse-test", "version": "1.0.0"}
        }
    }
    
    post_url = f"http://localhost:3002{endpoint_url}"
    print(f"Sending POST to: {post_url}")
    
    post_response = requests.post(post_url, json=init_request, headers={
        "Content-Type": "application/json",
        "Accept": "text/event-stream",
        "X-API-Key": "1234"
    })
    
    print(f"POST response status: {post_response.status_code}")
    
    # Wait for response via SSE
    print("Waiting for SSE response...")
    if response_received.wait(timeout=5):
        print(f"Got response: {json.dumps(sse_response, indent=2)}")
        return sse_response
    else:
        print("Timeout waiting for response")
        return None

if __name__ == "__main__":
    result = test_sse()
    if result:
        print("SSE test successful\!")
    else:
        print("SSE test failed\!")

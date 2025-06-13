#!/bin/bash

echo "Starting web server on http://localhost:8000"
echo "Press Ctrl+C to stop"
echo ""
echo "Opening browser..."
open "http://localhost:8000"

# Start the web server
python3 -m http.server 8000
#!/bin/bash
# Test script to verify environment variable setup

cd loxone-mcp-rust

echo "Testing environment variable setup..."
echo "4" | cargo run --bin loxone-mcp-setup 2>&1 | tail -n 30
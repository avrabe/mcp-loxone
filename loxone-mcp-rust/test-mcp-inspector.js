#!/usr/bin/env node

const { spawn } = require('child_process');
const readline = require('readline');

console.log('Testing MCP Server Connection...\n');

// Start the server
const server = spawn('./target/release/loxone-mcp-server', ['stdio'], {
  stdio: ['pipe', 'pipe', 'inherit'] // stdin, stdout, stderr
});

// Create readline interface for the server's stdout
const rl = readline.createInterface({
  input: server.stdout,
  output: process.stdout,
  terminal: false
});

// Send initialize request
const initRequest = {
  jsonrpc: '2.0',
  id: 1,
  method: 'initialize',
  params: {
    protocolVersion: '2024-11-05',
    capabilities: {
      tools: {}
    },
    clientInfo: {
      name: 'test-client',
      version: '1.0.0'
    }
  }
};

console.log('Sending initialize request...');
server.stdin.write(JSON.stringify(initRequest) + '\n');

// Handle responses
rl.on('line', (line) => {
  console.log('Received:', line);
  
  try {
    const response = JSON.parse(line);
    
    if (response.id === 1 && response.result) {
      console.log('\n✅ Initialize successful!');
      console.log('Server info:', response.result.serverInfo);
      console.log('Capabilities:', response.result.capabilities);
      
      // Send initialized notification
      const initializedNotification = {
        jsonrpc: '2.0',
        method: 'notifications/initialized'
      };
      
      console.log('\nSending initialized notification...');
      server.stdin.write(JSON.stringify(initializedNotification) + '\n');
      
      // List tools
      setTimeout(() => {
        const listToolsRequest = {
          jsonrpc: '2.0',
          id: 2,
          method: 'tools/list',
          params: {}
        };
        
        console.log('\nSending tools/list request...');
        server.stdin.write(JSON.stringify(listToolsRequest) + '\n');
      }, 100);
    } else if (response.id === 2 && response.result) {
      console.log('\n✅ Tools listed successfully!');
      console.log('Available tools:', response.result.tools.map(t => t.name));
      
      // Clean exit
      setTimeout(() => {
        server.kill();
        process.exit(0);
      }, 100);
    }
  } catch (e) {
    console.error('Failed to parse response:', e);
  }
});

// Handle server exit
server.on('exit', (code) => {
  console.log(`\nServer exited with code ${code}`);
  process.exit(code);
});

// Handle errors
server.on('error', (err) => {
  console.error('Failed to start server:', err);
  process.exit(1);
});
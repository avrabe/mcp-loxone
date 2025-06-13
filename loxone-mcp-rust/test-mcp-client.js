#!/usr/bin/env node

const { Client } = require('@modelcontextprotocol/sdk/client/index.js');
const { StdioClientTransport } = require('@modelcontextprotocol/sdk/client/stdio.js');

async function main() {
  console.log('Creating MCP client...');
  
  const transport = new StdioClientTransport({
    command: './target/release/loxone-mcp-server',
    args: ['stdio'],
    env: {
      ...process.env,
      // Don't override - let it use keychain credentials
    }
  });

  const client = new Client({
    name: 'test-client',
    version: '1.0.0'
  }, {
    capabilities: {
      tools: {}
    }
  });

  try {
    console.log('Connecting to server...');
    await client.connect(transport);
    console.log('✅ Connected successfully!');

    // Get server info
    const serverInfo = client.serverInfo;
    console.log('\nServer Info:', serverInfo);

    // List available tools
    console.log('\nListing tools...');
    const tools = await client.listTools();
    console.log('Available tools:');
    tools.tools.forEach(tool => {
      console.log(`  - ${tool.name}: ${tool.description}`);
    });

    // Test listing rooms
    console.log('\nTesting list_rooms tool...');
    const roomsResult = await client.callTool({
      name: 'list_rooms',
      arguments: {}
    });
    console.log('Rooms:', roomsResult.content[0].text);

    // Close connection
    await client.close();
    console.log('\n✅ Test completed successfully!');
  } catch (error) {
    console.error('❌ Error:', error);
    process.exit(1);
  }
}

main().catch(console.error);
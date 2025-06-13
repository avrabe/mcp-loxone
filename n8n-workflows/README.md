# n8n Workflows for Loxone MCP

This directory contains example n8n workflows that demonstrate advanced integration patterns with the Loxone MCP server.

## Available Workflows

### 1. Loxone MCP Server Workflows (`loxone-mcp-server-workflows.json`)

Advanced automation workflows that integrate directly with the MCP server:

- **Scene Manager**: Webhook-triggered scene automation (morning, evening, away, vacation modes)
- **Energy Monitor**: Real-time monitoring with threshold alerts and automatic load management
- **Security System**: Multi-mode security integration with zone control and panic features
- **Climate Control**: Intelligent temperature scheduling based on time of day and season

### 2. Loxone MCP Client Workflow (`loxone-mcp-client-workflow.json`)

Event-driven client that processes Loxone events and integrates with external services:

- **SSE Event Processing**: Real-time event handling from the MCP server
- **External Integrations**: Google Calendar, Slack, Email, SMS, Database
- **Analytics Engine**: Hourly reports with trends and recommendations
- **Multi-Channel Routing**: Intelligent notification routing based on priority

### 3. Simple Control Example (`../examples/n8n-loxone-workflow.json`)

Basic example demonstrating fundamental MCP operations:

- List all rooms
- Toggle lights
- Get device status
- Simple HTTP requests

## Setup Instructions

### 1. Import Workflow

1. Open n8n interface
2. Navigate to "Workflows"
3. Click "Import from File"
4. Select the downloaded `.json` file
5. Click "Import"

### 2. Configure Credentials

Create an HTTP Header Auth credential:

```
Name: Loxone MCP API Key
Header Name: Authorization
Header Value: Bearer YOUR_API_KEY_HERE
```

### 3. Update Server Endpoints

Update all HTTP Request nodes to point to your MCP server:

```
# For HTTP:
http://YOUR_MCP_SERVER:8080/sse

# For HTTPS:
https://YOUR_MCP_SERVER:8443/sse
```

### 4. Configure Webhooks (Optional)

If using webhook triggers, configure your n8n webhook URLs in the workflow nodes.

## Workflow Features

### Scene Management
- Room-specific control
- Batch operations for lights and blinds
- Time-based automation
- Vacation mode with random patterns

### Energy Management
- Real-time usage monitoring
- Threshold-based alerts
- Automatic load reduction
- Cost calculation and reporting

### Security Integration
- Multi-zone arming modes
- Panic alarm features
- Automatic lighting control
- Multi-channel alerting

### External Service Integration
- 350+ service connectors via n8n
- Database logging for analytics
- Scheduled reporting
- Custom JavaScript logic

## Customization

All workflows use JavaScript code nodes that can be easily customized:

1. **Modify Logic**: Edit the JavaScript in code nodes
2. **Add Services**: Use n8n's node library to add integrations
3. **Create Conditions**: Add IF nodes for conditional logic
4. **Schedule Tasks**: Use schedule triggers for automation

## Best Practices

1. **Test First**: Use the simple example to verify connectivity
2. **Start Small**: Begin with one workflow and expand
3. **Monitor Logs**: Check n8n execution logs for debugging
4. **Use Variables**: Store common values in workflow variables
5. **Error Handling**: Add error workflows for resilience

## Requirements

- n8n instance (self-hosted or cloud)
- Loxone MCP server running
- API key configured in MCP server
- Network connectivity between n8n and MCP server

## Troubleshooting

### Connection Issues
- Verify MCP server is accessible from n8n
- Check API key is correct
- Ensure proper URL format (http/https)

### Workflow Errors
- Check n8n execution logs
- Verify all credentials are configured
- Test individual nodes step by step

### Performance
- Use webhook triggers instead of polling
- Implement caching where appropriate
- Monitor n8n resource usage

## Additional Resources

- [n8n Documentation](https://docs.n8n.io)
- [n8n Community](https://community.n8n.io)
- [MCP Loxone Documentation](../README.md)

## License

These workflows are provided under the MIT License. See the main project LICENSE file for details.
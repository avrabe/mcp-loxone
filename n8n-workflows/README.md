# n8n Workflows for Loxone MCP Integration

This directory contains advanced n8n workflows that demonstrate the full capabilities of the Loxone MCP integration.

## Overview

The workflows implement a comprehensive home automation system with:
- **Event-driven automation** via SSE (Server-Sent Events)
- **Complex logic processing** for scenes, security, and energy management
- **External integrations** with popular services
- **Analytics and reporting** capabilities

## Workflows

### 1. Loxone MCP Server Workflows (`loxone-mcp-server-workflows.json`)

Advanced automation scenarios that leverage the MCP server:

#### Scene Manager
- **Endpoint**: `/webhook/loxone-scene-manager`
- **Scenes**: morning, evening, away, vacation
- **Features**:
  - Room-specific control
  - Time-based adjustments
  - Multi-device coordination

#### Energy Monitor
- **Endpoint**: `/webhook/loxone-energy-monitor`
- **Features**:
  - Real-time usage monitoring
  - Threshold-based alerts
  - Automatic load reduction
  - Cost tracking

#### Security System
- **Endpoint**: `/webhook/loxone-security-system`
- **Modes**: arm_away, arm_home, disarm, panic
- **Features**:
  - Zone-based arming
  - Automatic responses
  - Emergency notifications

#### Climate Control
- **Schedule**: Every 5 minutes
- **Features**:
  - Intelligent temperature adjustment
  - Time and season aware
  - Room-specific targeting

### 2. Loxone MCP Client Workflow (`loxone-mcp-client-workflow.json`)

Integration hub that processes events and connects to external services:

#### Event Processing
- Receives SSE events from Loxone
- Intelligent routing based on event type
- Automated response generation

#### External Integrations
- **Slack**: Real-time notifications
- **Google Calendar**: Event scheduling
- **Email**: Alert notifications
- **SMS**: Critical alerts via Twilio
- **Database**: Event logging and analytics

#### Analytics
- Hourly statistics generation
- Usage pattern analysis
- Automated recommendations

## Setup Instructions

### 1. Import Workflows

1. Open n8n interface
2. Go to Workflows → Import
3. Import each JSON file

### 2. Configure Credentials

Create the following credentials in n8n:

#### Loxone MCP API
- Type: HTTP Header Auth
- Name: `Loxone MCP API Key`
- Header Name: `Authorization`
- Header Value: `Bearer YOUR_API_KEY`

#### External Services (Optional)
- **Slack**: OAuth2 or Webhook URL
- **Google Calendar**: OAuth2
- **Gmail**: OAuth2
- **Twilio**: Account SID and Auth Token
- **PostgreSQL**: Database connection

### 3. Update Endpoints

Replace `http://localhost:8080` with your actual Loxone MCP server URL:
- In HTTP Request nodes
- In webhook callback URLs

### 4. Database Setup (Optional)

If using the analytics features, create the database table:

```sql
CREATE TABLE home_events (
  id SERIAL PRIMARY KEY,
  event_type VARCHAR(50),
  event_data JSONB,
  processed_at TIMESTAMP,
  actions_taken JSONB,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_event_type ON home_events(event_type);
CREATE INDEX idx_processed_at ON home_events(processed_at);
```

## Usage Examples

### Trigger a Scene

```bash
curl -X POST http://your-n8n-instance/webhook/loxone-scene-manager \
  -H "Content-Type: application/json" \
  -d '{
    "scene": "morning",
    "rooms": ["Bedroom", "Kitchen", "Bathroom"]
  }'
```

### Setup Energy Monitoring

```bash
curl -X POST http://your-n8n-instance/webhook/loxone-energy-monitor \
  -H "Content-Type: application/json" \
  -d '{
    "threshold": 5000,
    "duration": 30,
    "callback_url": "http://your-n8n-instance/webhook/loxone-energy-alert"
  }'
```

### Arm Security System

```bash
curl -X POST http://your-n8n-instance/webhook/loxone-security-system \
  -H "Content-Type: application/json" \
  -d '{
    "mode": "arm_away"
  }'
```

## Advanced Features

### Custom Logic

The workflows use JavaScript code nodes for complex logic:
- Dynamic scene generation
- Intelligent energy management
- Security threat assessment
- Adaptive climate control

### Event Correlation

Events are correlated across different systems:
- Security + Lighting
- Energy + Climate
- Presence + Scenes

### State Management

The system maintains state through:
- Database persistence
- Workflow variables
- External service integration

## Troubleshooting

### Common Issues

1. **Authentication Errors**
   - Verify API key in credentials
   - Check Bearer token format

2. **Connection Timeouts**
   - Ensure Loxone MCP server is running
   - Check network connectivity
   - Verify firewall rules

3. **Missing Events**
   - Check SSE connection status
   - Verify event subscriptions
   - Review server logs

### Debug Mode

Enable debug output in n8n:
1. Set environment variable: `N8N_LOG_LEVEL=debug`
2. Check workflow execution logs
3. Use "Test" mode for individual nodes

## Extension Ideas

1. **Weather Integration**
   - Connect to weather API
   - Adjust blinds based on sun position
   - Predictive climate control

2. **AI/ML Integration**
   - Pattern learning for occupancy
   - Predictive maintenance alerts
   - Energy usage optimization

3. **Voice Control**
   - Integration with Alexa/Google Home
   - Custom voice commands
   - Status announcements

4. **Mobile App Integration**
   - Push notifications
   - Remote control interface
   - Presence detection

## Performance Optimization

- Use webhook response immediately for fast acknowledgment
- Implement caching for frequently accessed data
- Batch similar operations
- Use async processing for non-critical tasks

## Security Considerations

- Always use HTTPS for webhooks
- Implement IP whitelisting where possible
- Rotate API keys regularly
- Log all security-related events
- Use encrypted connections for database
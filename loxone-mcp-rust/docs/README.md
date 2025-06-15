# Loxone MCP Rust Server Documentation

Welcome to the comprehensive documentation for the **Loxone MCP Rust Server** - a high-performance, production-ready implementation of the Model Context Protocol for Loxone home automation systems.

## 🎯 What is This?

The Loxone MCP Rust Server bridges the gap between modern AI assistants (like Claude) and your Loxone smart home system. It provides:

- **🤖 AI Integration**: Control your home with natural language through Claude Desktop
- **🔄 Workflow Automation**: Build complex automations with n8n
- **🌐 Universal Access**: REST API, WebSocket, and WASM deployment options
- **🛡️ Enterprise Security**: Production-grade security with rate limiting and validation
- **⚡ Blazing Performance**: Sub-10ms response times with Rust's zero-cost abstractions

## 📚 Documentation Overview

This documentation is organized into several sections:

### 🚀 [Getting Started](./QUICK_START.md)
New to the project? Start here! Learn how to install, configure, and run your first commands in minutes.

### 🎛️ [Configuration Guide](./CONFIGURATION.md)
**NEW!** Comprehensive configuration reference with:
- 📊 Complete environment variables list
- 🌳 Interactive decision trees for setup choices
- 🧙 [Web-based configuration wizard](./config-wizard.html)
- 🔐 Security levels and credential backends
- ⚡ Performance tuning scenarios

### 🔑 [API Reference](./API_REFERENCE.md)
Complete documentation for all 30+ MCP tools with examples, parameters, and authentication.

### 🛡️ [Security Guide](./SECURITY_ARCHITECTURE.md)
Multi-user API key management, role-based access control, and web UI for key administration.

### 🏗️ [Architecture](./ARCHITECTURE.md)
Deep dive into the system design, understanding the 12 core modules and how they work together.

### 🛠️ [Development](./DEVELOPMENT.md)
Everything you need to contribute, extend, or customize the server for your specific needs.

### 🚀 [Deployment](./DEPLOYMENT.md)
Production deployment strategies including Docker, Kubernetes, and edge computing with WASM.

### 🆘 [Troubleshooting](./TROUBLESHOOTING.md)
Solutions to common problems and debugging techniques.

## 🌟 Key Features

<table>
<tr>
<td width="50%">

### 🎛️ Complete Device Control
- **30+ MCP Tools** covering all device types
- **Batch Operations** for efficient control
- **Room-based Management** for logical grouping
- **Real-time Feedback** via WebSocket

</td>
<td width="50%">

### 🛡️ Production Security
- **Multi-user API Keys** with role-based access control
- **Web-based Key Management** UI at `/admin/keys`
- **Input Validation** on all parameters
- **Rate Limiting** with intelligent throttling
- **Audit Logging** for compliance
- **IP Whitelisting** for key restrictions

</td>
</tr>
<tr>
<td width="50%">

### ⚡ High Performance
- **Async I/O** with Tokio runtime
- **Connection Pooling** for efficiency
- **Smart Caching** reduces latency
- **WASM Support** for edge deployment

</td>
<td width="50%">

### 📊 Monitoring & Analytics
- **Real-time Dashboard** with metrics
- **Performance Profiling** built-in
- **Historical Data** with time-series storage
- **Alert System** for anomalies

</td>
</tr>
</table>

## 💻 Quick Examples

### Control Lights via Claude
```yaml
Human: Turn on all lights in the living room
Assistant: I'll turn on all lights in the living room for you.

[Calling control_room_devices tool...]
✓ Successfully turned on 4 lights in Living Room
```

### Climate Control with n8n
```json
{
  "tool": "set_room_temperature",
  "arguments": {
    "room": "Bedroom",
    "temperature": 22.5
  }
}
```

### Sensor Monitoring
```bash
curl -X POST http://localhost:3001/message \
  -H "X-API-Key: lmcp_monitor_001_abc123" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
      "name": "get_temperature_sensors",
      "arguments": {}
    },
    "id": 1
  }'
```

## 🏃 Quick Start Options

<div align="center">

| Method | Command | Time to Run |
|--------|---------|-------------|
| **🚀 Quick Script** | `curl -sSL https://install.sh \| bash` | 30 seconds |
| **🐳 Docker** | `docker-compose up` | 1 minute |
| **🦀 From Source** | `cargo run -- stdio` | 2 minutes |
| **🌐 WASM** | `make wasm && wasmtime serve` | 3 minutes |

</div>

## 📖 How to Use This Documentation

1. **New Users**: Start with [Quick Start](./QUICK_START.md) → [API Reference](./API_REFERENCE.md)
2. **Developers**: Check [Architecture](./ARCHITECTURE.md) → [Development](./DEVELOPMENT.md)
3. **DevOps**: Focus on [Deployment](./DEPLOYMENT.md) → [Security](./SECURITY_ARCHITECTURE.md)
4. **Troubleshooting**: Jump to [Troubleshooting](./TROUBLESHOOTING.md)

## 🔍 Search Tips

- Use the search bar (press `S`) to find specific topics
- Keywords: "tool", "api", "config", "error", "deploy"
- Check the [Glossary](./glossary.md) for terminology

## 🚦 System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| **Rust** | 1.70+ | Latest stable |
| **Memory** | 512MB | 1GB |
| **CPU** | 1 core | 2+ cores |
| **Loxone** | Miniserver | Any Miniserver |

## 🤝 Getting Help

- **📋 Documentation**: You're here!
- **💬 Discussions**: [GitHub Discussions](https://github.com/your-repo/discussions)
- **🐛 Issues**: [GitHub Issues](https://github.com/your-repo/issues)
- **📧 Contact**: [Email Support](mailto:support@example.com)

## 📊 Project Statistics

<div align="center">

| Metric | Value | Description |
|--------|-------|-------------|
| **📁 Source Files** | 183 | Comprehensive implementation |
| **🎛️ MCP Tools** | 30+ | Complete device coverage |
| **✅ Tests** | 226 | Extensive test suite |
| **📦 Dependencies** | 42 | Carefully selected |
| **⭐ Performance** | A+ | Production optimized |

</div>

---

<div align="center">

**Ready to get started?** → [🚀 Quick Start Guide](./QUICK_START.md)

*Built with ❤️ in Rust • Version 1.0.0*

</div>
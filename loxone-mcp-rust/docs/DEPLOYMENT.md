# Loxone MCP Deployment Guide

## Overview

This guide covers deploying the Loxone MCP server in various environments, from development to production.

## Deployment Options

### 1. Native Binary Deployment

#### Linux/macOS
```bash
# Build optimized release binary
cargo build --release

# Copy binary to deployment location
sudo cp target/release/loxone-mcp-server /usr/local/bin/

# Create systemd service (Linux)
sudo tee /etc/systemd/system/loxone-mcp.service > /dev/null <<EOF
[Unit]
Description=Loxone MCP Server
After=network.target

[Service]
Type=simple
User=loxone
Group=loxone
Environment="LOXONE_HOST=192.168.1.100"
Environment="LOXONE_USER=admin"
Environment="LOXONE_PASS=secure_password"
Environment="SECURITY_LEVEL=production"
ExecStart=/usr/local/bin/loxone-mcp-server http --port 3001
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# Enable and start service
sudo systemctl enable loxone-mcp
sudo systemctl start loxone-mcp
```

#### Windows
```powershell
# Build release binary
cargo build --release

# Install as Windows Service using NSSM
nssm install LoxoneMCP "C:\Program Files\LoxoneMCP\loxone-mcp-server.exe" "http --port 3001"
nssm set LoxoneMCP AppEnvironmentExtra LOXONE_HOST=192.168.1.100
nssm start LoxoneMCP
```

### 2. Docker Deployment

#### Basic Docker
```bash
# Build Docker image
docker build -t loxone-mcp:latest .

# Run container
docker run -d \
  --name loxone-mcp \
  -p 3001:3001 \
  -e LOXONE_HOST=192.168.1.100 \
  -e LOXONE_USER=admin \
  -e LOXONE_PASS=secure_password \
  -e SECURITY_LEVEL=production \
  -v /etc/loxone-mcp:/etc/loxone-mcp \
  --restart unless-stopped \
  loxone-mcp:latest
```

#### Docker Compose
```yaml
# docker-compose.yml
version: '3.8'

services:
  loxone-mcp:
    build: .
    container_name: loxone-mcp
    ports:
      - "3001:3001"
    environment:
      - LOXONE_HOST=192.168.1.100
      - LOXONE_USER=admin
      - LOXONE_PASS=${LOXONE_PASS}
      - SECURITY_LEVEL=production
      - ENABLE_LOXONE_STATS=1
    volumes:
      - ./config:/etc/loxone-mcp
      - ./keys:/var/lib/loxone-mcp/keys
      - ./logs:/var/log/loxone-mcp
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3001/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

### 3. Kubernetes Deployment

```yaml
# loxone-mcp-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: loxone-mcp
  labels:
    app: loxone-mcp
spec:
  replicas: 2
  selector:
    matchLabels:
      app: loxone-mcp
  template:
    metadata:
      labels:
        app: loxone-mcp
    spec:
      containers:
      - name: loxone-mcp
        image: your-registry/loxone-mcp:latest
        ports:
        - containerPort: 3001
        env:
        - name: LOXONE_HOST
          valueFrom:
            configMapKeyRef:
              name: loxone-config
              key: host
        - name: LOXONE_USER
          valueFrom:
            secretKeyRef:
              name: loxone-secrets
              key: username
        - name: LOXONE_PASS
          valueFrom:
            secretKeyRef:
              name: loxone-secrets
              key: password
        - name: SECURITY_LEVEL
          value: "production"
        livenessProbe:
          httpGet:
            path: /health
            port: 3001
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 3001
          initialDelaySeconds: 5
          periodSeconds: 5
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
---
apiVersion: v1
kind: Service
metadata:
  name: loxone-mcp
spec:
  selector:
    app: loxone-mcp
  ports:
    - protocol: TCP
      port: 3001
      targetPort: 3001
  type: LoadBalancer
```

### 4. WebAssembly (WASM) Deployment

#### Build WASM Binary
```bash
# Install WASM target
rustup target add wasm32-wasip2

# Build WASM binary
make wasm

# Verify binary size (should be ~2MB)
ls -lh target/wasm32-wasip2/release/loxone-mcp-server.wasm
```

#### Deploy to Wasmtime
```bash
# Install Wasmtime
curl https://wasmtime.dev/install.sh -sSf | bash

# Run WASM binary
wasmtime --serve target/wasm32-wasip2/release/loxone-mcp-server.wasm
```

#### Deploy to Cloudflare Workers
```javascript
// wrangler.toml
name = "loxone-mcp"
main = "target/wasm32-wasip2/release/loxone-mcp-server.wasm"
compatibility_date = "2024-01-01"

[vars]
LOXONE_HOST = "192.168.1.100"
SECURITY_LEVEL = "production"

[[kv_namespaces]]
binding = "KEYS"
id = "your-kv-namespace-id"
```

## Production Configuration

### 1. Environment Variables

```bash
# Required Configuration
export LOXONE_HOST=192.168.1.100        # Miniserver IP/hostname
export LOXONE_USER=admin                # Miniserver username
export LOXONE_PASS=secure_password      # Miniserver password

# Security Configuration
export SECURITY_LEVEL=production        # Enable all security features
export LOXONE_API_KEYS='[...]'         # Pre-configured API keys (JSON)

# Performance Configuration
export RUST_LOG=warn                    # Log level (debug/info/warn/error)
export LOXONE_CONNECTION_POOL_SIZE=50   # HTTP connection pool size
export LOXONE_REQUEST_TIMEOUT=30        # Request timeout in seconds

# Optional Features
export ENABLE_LOXONE_STATS=1           # Enable statistics collection
export INFLUXDB_URL=http://influx:8086 # InfluxDB for metrics
export INFLUXDB_TOKEN=your-token       # InfluxDB authentication
```

### 2. Reverse Proxy (Nginx)

```nginx
# /etc/nginx/sites-available/loxone-mcp
server {
    listen 443 ssl http2;
    server_name mcp.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/mcp.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/mcp.yourdomain.com/privkey.pem;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options "DENY" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
    limit_req zone=api burst=20 nodelay;

    location / {
        proxy_pass http://localhost:3001;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";

        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # Restrict admin endpoints
    location /admin {
        allow 10.0.0.0/8;
        allow 192.168.1.0/24;
        deny all;
        
        proxy_pass http://localhost:3001;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### 3. Security Hardening

#### API Key Configuration
```bash
# Generate production API keys
cargo run --bin loxone-mcp-keys -- generate --role admin --name "Production Admin" --expires 90
cargo run --bin loxone-mcp-keys -- generate --role operator --name "Automation System" --ip "10.0.0.0/8"
cargo run --bin loxone-mcp-keys -- generate --role monitor --name "Monitoring" --expires 365

# Export keys for backup
cargo run --bin loxone-mcp-keys -- export --format json --output /secure/backup/keys.json
```

#### Firewall Rules
```bash
# Allow only necessary ports
sudo ufw allow 22/tcp        # SSH
sudo ufw allow 443/tcp       # HTTPS
sudo ufw allow from 192.168.1.0/24 to any port 3001  # Internal access
sudo ufw enable
```

### 4. Monitoring & Logging

#### Prometheus Integration
```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'loxone-mcp'
    static_configs:
      - targets: ['localhost:3001']
    metrics_path: '/metrics'
```

#### Log Aggregation
```bash
# Configure rsyslog
echo "*.* @@logserver.local:514" >> /etc/rsyslog.conf

# Or use journald
journalctl -u loxone-mcp -f
```

### 5. High Availability

#### Load Balancing with HAProxy
```
# /etc/haproxy/haproxy.cfg
frontend loxone_mcp_front
    bind *:443 ssl crt /etc/ssl/certs/mcp.pem
    default_backend loxone_mcp_back

backend loxone_mcp_back
    balance roundrobin
    option httpchk GET /health
    server mcp1 10.0.0.10:3001 check
    server mcp2 10.0.0.11:3001 check
    server mcp3 10.0.0.12:3001 check
```

## Performance Tuning

### 1. System Limits
```bash
# /etc/security/limits.conf
loxone soft nofile 65536
loxone hard nofile 65536
loxone soft nproc 4096
loxone hard nproc 4096
```

### 2. Kernel Parameters
```bash
# /etc/sysctl.conf
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.ipv4.ip_local_port_range = 1024 65535
net.ipv4.tcp_tw_reuse = 1
```

### 3. Application Tuning
```bash
# Optimize for performance
export LOXONE_CONNECTION_POOL_SIZE=100
export LOXONE_REQUEST_TIMEOUT=60
export RUST_LOG=warn
export LOXONE_CACHE_TTL=300
```

## Backup & Recovery

### 1. Backup Script
```bash
#!/bin/bash
# backup-loxone-mcp.sh

BACKUP_DIR="/backup/loxone-mcp/$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"

# Backup configuration
cp -r /etc/loxone-mcp "$BACKUP_DIR/config"

# Backup API keys
cargo run --bin loxone-mcp-keys -- export --output "$BACKUP_DIR/keys.json"

# Backup logs
cp -r /var/log/loxone-mcp "$BACKUP_DIR/logs"

# Compress backup
tar -czf "$BACKUP_DIR.tar.gz" "$BACKUP_DIR"
rm -rf "$BACKUP_DIR"

# Retain last 30 days
find /backup/loxone-mcp -name "*.tar.gz" -mtime +30 -delete
```

### 2. Restore Procedure
```bash
# Stop service
sudo systemctl stop loxone-mcp

# Extract backup
tar -xzf /backup/loxone-mcp/20240115.tar.gz -C /

# Import API keys
cargo run --bin loxone-mcp-keys -- import /backup/loxone-mcp/20240115/keys.json

# Start service
sudo systemctl start loxone-mcp
```

## Troubleshooting Deployment

### Common Issues

1. **Connection Refused**
   ```bash
   # Check if service is running
   systemctl status loxone-mcp
   
   # Check ports
   netstat -tlnp | grep 3001
   ```

2. **High Memory Usage**
   ```bash
   # Adjust connection pool
   export LOXONE_CONNECTION_POOL_SIZE=25
   
   # Enable memory profiling
   export RUST_LOG=debug
   ```

3. **Slow Response Times**
   ```bash
   # Check network latency to Miniserver
   ping -c 10 192.168.1.100
   
   # Enable performance monitoring
   export LOXONE_PERFORMANCE_MODE=development
   ```

### Health Checks

```bash
# Basic health check
curl http://localhost:3001/health

# Detailed status
curl -H "X-API-Key: your-key" http://localhost:3001/admin/status

# Rate limit status
curl -H "X-API-Key: your-key" http://localhost:3001/admin/rate-limits
```

## Security Checklist

- [ ] Use HTTPS in production (TLS 1.2+)
- [ ] Configure firewall rules
- [ ] Set strong API keys with expiration
- [ ] Enable rate limiting
- [ ] Configure IP whitelisting for admin endpoints
- [ ] Regular security updates
- [ ] Enable audit logging
- [ ] Backup API keys securely
- [ ] Monitor for suspicious activity
- [ ] Test disaster recovery procedures
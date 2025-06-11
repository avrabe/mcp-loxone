# Production HTTPS Setup Guide

## Reverse Proxy with Let's Encrypt (Recommended)

### 1. Nginx + Certbot Setup

#### nginx.conf
```nginx
server {
    listen 80;
    server_name your-domain.com;
    
    # Redirect all HTTP to HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name your-domain.com;
    
    # SSL certificates (managed by Certbot)
    ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;
    
    # SSL security settings
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-RSA-AES256-GCM-SHA384:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;
    
    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options DENY always;
    add_header X-Content-Type-Options nosniff always;
    add_header X-XSS-Protection "1; mode=block" always;
    
    # Proxy to Loxone MCP SSE server
    location / {
        proxy_pass http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        
        # For Server-Sent Events
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 24h;
    }
    
    # Health check (bypass auth)
    location /health {
        proxy_pass http://127.0.0.1:8000/health;
        access_log off;
    }
}
```

#### Docker Compose with SSL Termination
```yaml
# docker-compose.prod.yml
version: '3.8'

services:
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/conf.d/default.conf
      - ./ssl:/etc/ssl
      - letsencrypt_data:/etc/letsencrypt
    depends_on:
      - loxone-mcp-sse
    restart: unless-stopped

  certbot:
    image: certbot/certbot
    volumes:
      - letsencrypt_data:/etc/letsencrypt
      - ./ssl:/etc/ssl
    command: certonly --webroot --webroot-path=/var/www/certbot --email your-email@domain.com --agree-tos --no-eff-email -d your-domain.com

  loxone-mcp-sse:
    build:
      context: .
      dockerfile: Dockerfile.sse
    environment:
      - LOXONE_SSE_HOST=0.0.0.0  # Bind to all interfaces for Docker
      - LOXONE_SSE_PORT=8000
      - LOXONE_SSE_REQUIRE_AUTH=true
      - LOXONE_HOST=${LOXONE_HOST}
      - LOXONE_USER=${LOXONE_USER}
      - LOXONE_PASS=${LOXONE_PASS}
      - LOXONE_SSE_API_KEY=${LOXONE_SSE_API_KEY}
    expose:
      - "8000"
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  letsencrypt_data:
```

### 2. Traefik with Automatic SSL (Alternative)

#### docker-compose.traefik.yml
```yaml
version: '3.8'

services:
  traefik:
    image: traefik:v3.0
    command:
      - "--api.dashboard=true"
      - "--providers.docker=true"
      - "--providers.docker.exposedbydefault=false"
      - "--entrypoints.web.address=:80"
      - "--entrypoints.websecure.address=:443"
      - "--certificatesresolvers.letsencrypt.acme.email=your-email@domain.com"
      - "--certificatesresolvers.letsencrypt.acme.storage=/acme.json"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge=true"
      - "--certificatesresolvers.letsencrypt.acme.httpchallenge.entrypoint=web"
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - traefik_acme:/acme.json
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.api.rule=Host(`traefik.your-domain.com`)"
      - "traefik.http.routers.api.tls.certresolver=letsencrypt"

  loxone-mcp-sse:
    build:
      context: .
      dockerfile: Dockerfile.sse
    environment:
      - LOXONE_SSE_HOST=0.0.0.0
      - LOXONE_SSE_PORT=8000
      - LOXONE_SSE_REQUIRE_AUTH=true
      - LOXONE_HOST=${LOXONE_HOST}
      - LOXONE_USER=${LOXONE_USER}
      - LOXONE_PASS=${LOXONE_PASS}
      - LOXONE_SSE_API_KEY=${LOXONE_SSE_API_KEY}
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.loxone-sse.rule=Host(`loxone.your-domain.com`)"
      - "traefik.http.routers.loxone-sse.tls.certresolver=letsencrypt"
      - "traefik.http.services.loxone-sse.loadbalancer.server.port=8000"
      # Redirect HTTP to HTTPS
      - "traefik.http.routers.loxone-sse-http.rule=Host(`loxone.your-domain.com`)"
      - "traefik.http.routers.loxone-sse-http.entrypoints=web"
      - "traefik.http.routers.loxone-sse-http.middlewares=redirect-to-https"
      - "traefik.http.middlewares.redirect-to-https.redirectscheme.scheme=https"

volumes:
  traefik_acme:
```

## Cloud Deployment Options

### AWS Application Load Balancer
- Automatic SSL certificate management
- Integration with AWS Certificate Manager
- Health checks and auto-scaling

### Cloudflare
- Free SSL certificates
- DDoS protection
- Global CDN

### Docker Cloud Services
- Most cloud providers offer managed SSL
- DigitalOcean App Platform
- Google Cloud Run
- Azure Container Instances
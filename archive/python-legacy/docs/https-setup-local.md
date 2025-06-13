# Local HTTPS Setup Guide

## Using mkcert for Local Development

### 1. Install mkcert
```bash
# macOS
brew install mkcert
brew install nss  # for Firefox

# Linux
wget -O mkcert https://github.com/FiloSottile/mkcert/releases/latest/download/mkcert-v1.4.4-linux-amd64
chmod +x mkcert
sudo mv mkcert /usr/local/bin/
```

### 2. Create Local CA
```bash
# Create local certificate authority
mkcert -install

# Generate certificate for local domains
mkcert localhost 127.0.0.1 ::1 loxone-mcp.local
```

### 3. Update SSE Server for HTTPS
Create `src/loxone_mcp/ssl_config.py`:
```python
"""SSL configuration for HTTPS support."""

import os
import ssl
from pathlib import Path

def create_ssl_context() -> ssl.SSLContext | None:
    """Create SSL context for HTTPS."""
    cert_file = os.getenv("LOXONE_SSL_CERT", "localhost+2.pem")
    key_file = os.getenv("LOXONE_SSL_KEY", "localhost+2-key.pem")
    
    if not (Path(cert_file).exists() and Path(key_file).exists()):
        return None
    
    context = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
    context.load_cert_chain(cert_file, key_file)
    return context
```

### 4. Environment Variables
```bash
# .env for development
LOXONE_SSE_USE_HTTPS=true
LOXONE_SSL_CERT=localhost+2.pem
LOXONE_SSL_KEY=localhost+2-key.pem
LOXONE_SSE_PORT=8443
```
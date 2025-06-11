#!/bin/bash
# Setup HTTPS for Loxone MCP SSE server

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CERT_DIR="$PROJECT_ROOT/certs"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔒 Loxone MCP HTTPS Setup${NC}"
echo "=================================="

# Check if we're in the right directory
if [[ ! -f "$PROJECT_ROOT/pyproject.toml" ]]; then
    echo -e "${RED}❌ Error: Please run this script from the project root${NC}"
    exit 1
fi

# Create certs directory
mkdir -p "$CERT_DIR"

echo ""
echo "Choose HTTPS setup method:"
echo "1. Development: Self-signed certificate"
echo "2. Development: mkcert (requires installation)"
echo "3. Production: Use existing certificates"
echo "4. Production: Generate CSR for CA signing"
echo "5. Show current HTTPS configuration"

read -p "Enter your choice (1-5): " choice

case $choice in
    1)
        echo -e "\n${YELLOW}Setting up self-signed certificate...${NC}"
        
        # Generate private key
        openssl genrsa -out "$CERT_DIR/server.key" 2048
        
        # Generate certificate signing request
        openssl req -new -key "$CERT_DIR/server.key" -out "$CERT_DIR/server.csr" \
            -subj "/C=US/ST=Development/L=Local/O=LoxoneMCP/CN=localhost"
        
        # Generate self-signed certificate
        openssl x509 -req -days 365 -in "$CERT_DIR/server.csr" \
            -signkey "$CERT_DIR/server.key" -out "$CERT_DIR/server.crt" \
            -extensions v3_req -extfile <(
cat <<EOF
[v3_req]
basicConstraints = CA:FALSE
keyUsage = nonRepudiation, digitalSignature, keyEncipherment
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = loxone-mcp.local
IP.1 = 127.0.0.1
IP.2 = ::1
EOF
)
        
        echo -e "${GREEN}✅ Self-signed certificate generated${NC}"
        echo "Certificate: $CERT_DIR/server.crt"
        echo "Private Key: $CERT_DIR/server.key"
        echo ""
        echo -e "${YELLOW}⚠️  Add to your hosts file for local domain:${NC}"
        echo "127.0.0.1 loxone-mcp.local"
        ;;
        
    2)
        echo -e "\n${YELLOW}Setting up mkcert certificate...${NC}"
        
        # Check if mkcert is installed
        if ! command -v mkcert &> /dev/null; then
            echo -e "${RED}❌ mkcert not found${NC}"
            echo "Install mkcert:"
            echo "  macOS: brew install mkcert"
            echo "  Linux: Download from https://github.com/FiloSottile/mkcert/releases"
            exit 1
        fi
        
        # Install local CA if not already done
        mkcert -install
        
        # Generate certificate
        cd "$CERT_DIR"
        mkcert -cert-file server.crt -key-file server.key \
            localhost 127.0.0.1 ::1 loxone-mcp.local
        
        echo -e "${GREEN}✅ mkcert certificate generated${NC}"
        echo "Certificate: $CERT_DIR/server.crt"
        echo "Private Key: $CERT_DIR/server.key"
        ;;
        
    3)
        echo -e "\n${YELLOW}Using existing certificates...${NC}"
        read -p "Enter path to certificate file (.crt or .pem): " cert_path
        read -p "Enter path to private key file (.key): " key_path
        
        if [[ ! -f "$cert_path" ]]; then
            echo -e "${RED}❌ Certificate file not found: $cert_path${NC}"
            exit 1
        fi
        
        if [[ ! -f "$key_path" ]]; then
            echo -e "${RED}❌ Private key file not found: $key_path${NC}"
            exit 1
        fi
        
        # Copy certificates to certs directory
        cp "$cert_path" "$CERT_DIR/server.crt"
        cp "$key_path" "$CERT_DIR/server.key"
        
        echo -e "${GREEN}✅ Certificates copied${NC}"
        ;;
        
    4)
        echo -e "\n${YELLOW}Generating CSR for CA signing...${NC}"
        read -p "Enter your domain name (e.g., loxone.yourdomain.com): " domain
        
        # Generate private key
        openssl genrsa -out "$CERT_DIR/server.key" 2048
        
        # Generate CSR
        openssl req -new -key "$CERT_DIR/server.key" -out "$CERT_DIR/server.csr" \
            -subj "/C=US/ST=Production/L=Server/O=LoxoneMCP/CN=$domain"
        
        echo -e "${GREEN}✅ CSR generated${NC}"
        echo "Private Key: $CERT_DIR/server.key"
        echo "CSR: $CERT_DIR/server.csr"
        echo ""
        echo "Send the CSR to your Certificate Authority for signing."
        echo "Once signed, save the certificate as: $CERT_DIR/server.crt"
        ;;
        
    5)
        echo -e "\n${BLUE}Current HTTPS Configuration:${NC}"
        echo "CERT_DIR: $CERT_DIR"
        
        if [[ -f "$CERT_DIR/server.crt" ]]; then
            echo -e "${GREEN}✅ Certificate found${NC}: $CERT_DIR/server.crt"
            echo "Certificate details:"
            openssl x509 -in "$CERT_DIR/server.crt" -text -noout | grep -E "(Subject:|Not Before|Not After|DNS:|IP Address:)" || true
        else
            echo -e "${RED}❌ Certificate not found${NC}"
        fi
        
        if [[ -f "$CERT_DIR/server.key" ]]; then
            echo -e "${GREEN}✅ Private key found${NC}: $CERT_DIR/server.key"
        else
            echo -e "${RED}❌ Private key not found${NC}"
        fi
        
        echo ""
        echo "Environment variables for HTTPS:"
        echo "export LOXONE_SSE_USE_HTTPS=true"
        echo "export LOXONE_SSL_CERT=$CERT_DIR/server.crt"
        echo "export LOXONE_SSL_KEY=$CERT_DIR/server.key"
        echo "export LOXONE_SSL_PORT=8443"
        exit 0
        ;;
        
    *)
        echo -e "${RED}❌ Invalid choice${NC}"
        exit 1
        ;;
esac

# Set appropriate permissions
chmod 600 "$CERT_DIR/server.key"
chmod 644 "$CERT_DIR/server.crt"

echo ""
echo -e "${GREEN}🎉 HTTPS setup complete!${NC}"
echo ""
echo "To enable HTTPS, set these environment variables:"
echo "export LOXONE_SSE_USE_HTTPS=true"
echo "export LOXONE_SSL_CERT=$CERT_DIR/server.crt"
echo "export LOXONE_SSL_KEY=$CERT_DIR/server.key"
echo "export LOXONE_SSL_PORT=8443"
echo ""
echo "Or create a .env file:"
cat > "$PROJECT_ROOT/.env.https.example" << EOF
# HTTPS Configuration for Loxone MCP SSE Server
LOXONE_SSE_USE_HTTPS=true
LOXONE_SSL_CERT=$CERT_DIR/server.crt
LOXONE_SSL_KEY=$CERT_DIR/server.key
LOXONE_SSL_PORT=8443

# Loxone Configuration (required)
LOXONE_HOST=192.168.1.100
LOXONE_USER=your-username
LOXONE_PASS=your-password
LOXONE_SSE_API_KEY=your-api-key

# SSE Server Configuration
LOXONE_SSE_REQUIRE_AUTH=true
LOXONE_SSE_HOST=0.0.0.0
LOXONE_SSE_PORT=8000
EOF

echo "Example configuration saved to: .env.https.example"
echo ""
echo "Next steps:"
echo "1. Copy .env.https.example to .env and edit with your credentials"
echo "2. Start the server: uvx --from . loxone-mcp-sse"
echo "3. Or with Docker: docker-compose -f docker-compose.ssl.yml up"
echo ""
echo -e "${BLUE}📚 Documentation:${NC}"
echo "- Local setup: docs/https-setup-local.md"
echo "- Production: docs/https-setup-production.md"
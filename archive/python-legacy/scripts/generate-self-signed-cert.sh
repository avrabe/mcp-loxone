#!/bin/bash
# Generate self-signed certificate for local development

set -e

CERT_DIR="certs"
DOMAIN="loxone-mcp.local"

# Create certs directory
mkdir -p "$CERT_DIR"

# Generate private key
openssl genrsa -out "$CERT_DIR/server.key" 2048

# Generate certificate signing request
openssl req -new -key "$CERT_DIR/server.key" -out "$CERT_DIR/server.csr" \
    -subj "/C=US/ST=Development/L=Local/O=LoxoneMCP/CN=$DOMAIN"

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
DNS.2 = $DOMAIN
IP.1 = 127.0.0.1
IP.2 = ::1
EOF
)

echo "✅ Self-signed certificate generated:"
echo "   Certificate: $CERT_DIR/server.crt"
echo "   Private Key: $CERT_DIR/server.key"
echo ""
echo "⚠️  Add to your hosts file:"
echo "   127.0.0.1 $DOMAIN"
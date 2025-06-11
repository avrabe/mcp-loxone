"""SSL/HTTPS configuration for Loxone MCP SSE server.

SPDX-License-Identifier: MIT
Copyright (c) 2025 Ralf Anton Beier
"""

import logging
import os
import ssl
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)

# SSL Configuration
SSL_ENABLED = os.getenv("LOXONE_SSE_USE_HTTPS", "false").lower() == "true"
SSL_CERT_FILE = os.getenv("LOXONE_SSL_CERT", "certs/server.crt")
SSL_KEY_FILE = os.getenv("LOXONE_SSL_KEY", "certs/server.key")
SSL_PORT = int(os.getenv("LOXONE_SSL_PORT", "8443"))


def create_ssl_context() -> ssl.SSLContext | None:
    """Create SSL context for HTTPS support.

    Returns:
        SSL context if certificates are available and valid, None otherwise
    """
    if not SSL_ENABLED:
        logger.debug("HTTPS disabled via LOXONE_SSE_USE_HTTPS=false")
        return None

    cert_path = Path(SSL_CERT_FILE)
    key_path = Path(SSL_KEY_FILE)

    if not cert_path.exists():
        logger.warning(f"SSL certificate not found: {cert_path}")
        logger.warning("HTTPS disabled - run scripts/generate-self-signed-cert.sh for development")
        return None

    if not key_path.exists():
        logger.warning(f"SSL private key not found: {key_path}")
        logger.warning("HTTPS disabled - check SSL_KEY_FILE configuration")
        return None

    try:
        # Create SSL context with secure defaults
        context = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
        context.load_cert_chain(str(cert_path), str(key_path))

        # Security settings
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        context.set_ciphers('ECDHE+AESGCM:ECDHE+CHACHA20:DHE+AESGCM:DHE+CHACHA20:!aNULL:!MD5:!DSS')

        logger.info("✅ SSL context created successfully")
        logger.info(f"   Certificate: {cert_path}")
        logger.info(f"   Private Key: {key_path}")

        return context

    except ssl.SSLError as e:
        logger.error(f"SSL configuration error: {e}")
        logger.error("HTTPS disabled due to SSL configuration issues")
        return None
    except Exception as e:
        logger.error(f"Failed to create SSL context: {e}")
        return None


def get_ssl_config() -> dict[str, Any]:
    """Get SSL configuration for FastMCP/uvicorn.

    Returns:
        Dictionary with SSL configuration parameters
    """
    ssl_context = create_ssl_context()

    if ssl_context:
        return {
            "ssl_context": ssl_context,
            "port": SSL_PORT,
            "scheme": "https"
        }
    else:
        return {
            "ssl_context": None,
            "port": int(os.getenv("LOXONE_SSE_PORT", "8000")),
            "scheme": "http"
        }


def validate_ssl_setup() -> tuple[bool, str]:
    """Validate SSL setup and provide helpful error messages.

    Returns:
        Tuple of (is_valid, message)
    """
    if not SSL_ENABLED:
        return True, "HTTPS disabled"

    cert_path = Path(SSL_CERT_FILE)
    key_path = Path(SSL_KEY_FILE)

    if not cert_path.exists():
        return False, f"""
SSL certificate not found: {cert_path}

To set up HTTPS for development:
1. Run: ./scripts/generate-self-signed-cert.sh
2. Or use mkcert: mkcert localhost 127.0.0.1 ::1
3. Set environment variables:
   export LOXONE_SSL_CERT=path/to/cert.pem
   export LOXONE_SSL_KEY=path/to/key.pem

For production:
1. Use Let's Encrypt with reverse proxy (recommended)
2. Or obtain certificates from your CA
3. Set LOXONE_SSL_CERT and LOXONE_SSL_KEY paths
"""

    if not key_path.exists():
        return False, f"SSL private key not found: {key_path}"

    try:
        # Test loading the certificate
        context = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
        context.load_cert_chain(str(cert_path), str(key_path))
        return True, "SSL configuration valid"
    except ssl.SSLError as e:
        return False, f"SSL configuration error: {e}"
    except Exception as e:
        return False, f"SSL validation failed: {e}"


def get_server_urls(host: str) -> list[str]:
    """Get server URLs for both HTTP and HTTPS if available.

    Args:
        host: Server host address

    Returns:
        List of server URLs
    """
    urls = []
    ssl_config = get_ssl_config()

    if ssl_config["ssl_context"]:
        # HTTPS available
        urls.append(f"https://{host}:{ssl_config['port']}")
        # Also show HTTP port if different
        http_port = int(os.getenv("LOXONE_SSE_PORT", "8000"))
        if http_port != ssl_config["port"]:
            urls.append(f"http://{host}:{http_port} (redirects to HTTPS)")
    else:
        # HTTP only
        http_port = ssl_config["port"]
        urls.append(f"http://{host}:{http_port}")

    return urls

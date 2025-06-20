# 🚀 Loxone MCP Rust - Local Quick Start

## 1-Minute Setup

### 🧪 Option 0: Mock Server (Optional - Kein echter Miniserver nötig!)

```bash
# NOTE: Mock server binary currently not implemented
# Use Option A with real Loxone hardware instead

# Alternative: Use the mock client in test code
# See src/mock/mod.rs for MockLoxoneClient implementation
```

### Option A: Environment Variables (Schnellster Start)

```bash
# 1. Setze deine Loxone Credentials
export LOXONE_USERNAME="admin"
export LOXONE_PASSWORD="dein-passwort"
export LOXONE_HOST="192.168.1.100"  # Deine Miniserver IP

# 2. Server starten
cargo run --bin loxone-mcp-server
```

### Option B: .env Datei (Empfohlen für Entwicklung)

```bash
# 1. Kopiere die Beispiel-Datei
cp .env.example .env

# 2. Editiere .env mit deinen Werten
nano .env  # oder: code .env

# 3. Lade die Variablen und starte
source .env
cargo run --bin loxone-mcp-server
```

### Option C: Keychain Setup (Einmalig, Persistiert)

```bash
# Interaktives Setup - speichert sicher im macOS Keychain
cargo run --bin loxone-mcp-setup

# Oder spezifisches Backend wählen:
cargo run --bin loxone-mcp-setup --backend keychain
cargo run --bin loxone-mcp-setup --backend infisical
cargo run --bin loxone-mcp-setup --backend environment

# Beim nächsten Start werden Credentials automatisch geladen:
cargo run --bin loxone-mcp-server
```

## Server Discovery

Falls du die IP deines Miniservers nicht kennst:

```bash
# Automatische Suche im Netzwerk
cargo run --bin loxone-mcp-setup
# Wähle "Automatic discovery" wenn gefragt
```

## Testen

```bash
# Verbindung testen
cargo run --bin loxone-mcp-test-connection

# MCP Inspector starten (für Debugging)
npx @modelcontextprotocol/inspector cargo run --bin loxone-mcp-server
```

## Probleme?

- **"No credentials found"**: Stelle sicher dass die Environment Variables gesetzt sind
- **"Connection refused"**: Prüfe die IP-Adresse und ob der Miniserver erreichbar ist
- **"401 Unauthorized"**: Username oder Passwort falsch

## Nächste Schritte

- Für Team-Umgebungen: Siehe [INFISICAL_SETUP.md](./INFISICAL_SETUP.md)
- Für Production: Siehe [README.md](./README.md)
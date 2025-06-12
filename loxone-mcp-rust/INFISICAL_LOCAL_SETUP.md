# 🏠 Lokale Infisical-Instanz einrichten

## Option 1: Docker (Einfachste Lösung)

### 1. Docker Compose Setup

```bash
# Infisical Docker Compose herunterladen
curl -o docker-compose.yml https://raw.githubusercontent.com/Infisical/infisical/main/docker-compose.yml

# Oder manuell erstellen:
cat > docker-compose.yml << 'EOF'
version: '3.8'

services:
  infisical:
    image: infisical/infisical:latest
    ports:
      - "8080:8080"
    environment:
      - NODE_ENV=production
      - DB_CONNECTION_URI=postgresql://infisical:password@postgres:5432/infisical
      - REDIS_URL=redis://redis:6379
      - JWT_SECRET=your-jwt-secret-here
      - ENCRYPTION_KEY=your-32-char-encryption-key-here
      - SMTP_HOST=smtp.gmail.com
      - SMTP_PORT=587
      - SMTP_SECURE=false
      - SMTP_FROM_ADDRESS=noreply@yourcompany.com
      - SMTP_FROM_NAME=Infisical
    depends_on:
      - postgres
      - redis

  postgres:
    image: postgres:13
    environment:
      - POSTGRES_USER=infisical
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=infisical
    volumes:
      - postgres_data:/var/lib/postgresql/data

  redis:
    image: redis:7
    volumes:
      - redis_data:/data

volumes:
  postgres_data:
  redis_data:
EOF
```

### 2. Starten

```bash
# Infisical starten
docker-compose up -d

# Logs checken
docker-compose logs -f infisical

# Öffne: http://localhost:8080
```

### 3. Ersten Admin-User erstellen

```bash
# Im Browser öffnen: http://localhost:8080
# Registriere den ersten Admin-Account
# Erstelle ein Projekt
```

## Option 2: Direkt mit Node.js

```bash
# Infisical CLI installieren
npm install -g @infisical/cli

# Repository klonen
git clone https://github.com/Infisical/infisical.git
cd infisical

# Dependencies installieren
npm install

# Environment Variables setzen
cp .env.example .env
# Editiere .env mit deinen Werten

# Datenbank setup
npm run migration:latest

# Server starten
npm run dev
```

## Rust MCP Server konfigurieren

### 1. Umgebungsvariablen setzen

```bash
# Für lokale Infisical-Instanz (Docker):
export INFISICAL_HOST="http://localhost:8080"        # Lokale Docker-Instanz
export INFISICAL_PROJECT_ID="proj_abc123def456"      # Aus Infisical Dashboard
export INFISICAL_CLIENT_ID="st.client123def456"      # Erstelle Machine Identity  
export INFISICAL_CLIENT_SECRET="st.secret456ghi789"  # Service Token
export INFISICAL_ENVIRONMENT="dev"

# Für selbst-gehostete Instanz:
export INFISICAL_HOST="https://infisical.your-company.com"
export INFISICAL_PROJECT_ID="proj_company123def456"
export INFISICAL_CLIENT_ID="st.company456ghi789"
export INFISICAL_CLIENT_SECRET="st.company789jkl012"
export INFISICAL_ENVIRONMENT="production"
```

### 2. Service Token in lokaler Infisical erstellen

1. **Öffne http://localhost:8080**
2. **Erstelle ein Projekt** (z.B. "loxone-local")
3. **Gehe zu Settings → Machine Identities**
4. **Create Machine Identity**:
   - Name: `loxone-mcp-rust`
   - Role: Admin (oder custom mit read/write)
5. **Kopiere die Client ID und Secret**

### 3. Secrets in Infisical hinzufügen

Im Infisical Dashboard:
1. **Gehe zu Secrets**
2. **Add Secret**:
   - `LOXONE_USER` = `admin`
   - `LOXONE_PASS` = `your-password`
   - `LOXONE_HOST` = `192.168.1.100`

### 4. Testen

```bash
# Teste die Verbindung
cargo run --bin loxone-mcp-setup

# Du solltest sehen:
# ✅ Infisical credential backend enabled
# ✅ Credentials loaded for user: admin
```

## Troubleshooting

### Problem: "Connection refused"
```bash
# Prüfe ob Infisical läuft:
curl http://localhost:8080/api/status

# Docker logs checken:
docker-compose logs infisical
```

### Problem: "Invalid credentials"
```bash
# Prüfe die Umgebungsvariablen:
env | grep INFISICAL

# Teste API direkt:
curl -H "Authorization: Bearer $INFISICAL_CLIENT_SECRET" \
     http://localhost:8080/api/v1/auth/universal-auth/login
```

### Problem: "Project not found"
- Prüfe ob die PROJECT_ID korrekt ist
- Stelle sicher dass die Machine Identity Zugriff auf das Projekt hat

## Vorteile lokale Instanz

✅ Vollständige Kontrolle über deine Secrets  
✅ Keine externe Abhängigkeiten  
✅ Offline-Entwicklung möglich  
✅ Custom Policies und Roles  
✅ Audit Logs bleiben lokal  
✅ Kostenlos ohne Limits
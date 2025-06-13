# Infisical Setup für Loxone MCP

## Was ist Infisical?

Infisical ist ein Open-Source Secret Management System, das es Teams erlaubt, Credentials sicher zu teilen. Die Rust-Implementation hat Infisical bereits integriert - du musst nichts extra installieren!

## Schnellstart (5 Minuten)

### 1. Infisical Account erstellen

1. Gehe zu https://app.infisical.com/signup
2. Erstelle einen kostenlosen Account (kein Kreditkarte nötig)

### 2. Projekt anlegen

1. Klicke auf "Create Project"
2. Name: `loxone-home` (oder wie du willst)
3. Environment: Lass erstmal auf "Development"

### 3. Service Token erstellen

1. Gehe zu: **Settings → Service Tokens**
2. Klicke "Create Service Token"
3. Name: `loxone-mcp-rust`
4. Scopes: Wähle aus:
   - `secrets:read`
   - `secrets:write`
5. Expiry: "Never" oder wie lange du willst
6. Klicke "Create"

### 4. Credentials kopieren

Nach dem Erstellen siehst du 3 wichtige Werte. Kopiere diese SOFORT, sie werden nur einmal angezeigt:

```bash
# Kopiere diese Befehle und ersetze mit deinen echten Werten:
export INFISICAL_PROJECT_ID="65f8e2c8a8b7d9001c4f2a3b"        # Aus der URL oder Settings
export INFISICAL_CLIENT_ID="6f4d8e91-3a2b-4c5d-9e7f-1a2b3c4d5e6f"     # Machine Identity ID
export INFISICAL_CLIENT_SECRET="st.abc123def456ghi789jkl012mno345pqr678stu901vwx234yz"  # Token
export INFISICAL_ENVIRONMENT="dev"
```

### 5. Loxone Credentials in Infisical speichern

1. Gehe zu deinem Projekt Dashboard
2. Klicke auf "Secrets" 
3. Füge diese Secrets hinzu:
   - Key: `LOXONE_USERNAME`, Value: `dein-loxone-username`
   - Key: `LOXONE_PASSWORD`, Value: `dein-loxone-passwort`
   - Key: `LOXONE_HOST`, Value: `192.168.1.100` (deine Miniserver IP)

### 6. Testen

```bash
# Environment Variables setzen (aus Schritt 4)
export INFISICAL_PROJECT_ID="..."
export INFISICAL_CLIENT_ID="..."
export INFISICAL_CLIENT_SECRET="..."

# Server starten
cargo run --release

# Du solltest sehen:
# ✅ Infisical credential backend enabled
# ✅ Credentials loaded for user: dein-username
```

## Für dein Team

Teile nur die 4 Environment Variables mit deinem Team:
```bash
# In .env.example oder README:
INFISICAL_PROJECT_ID="65f8e2c8a8b7d9001c4f2a3b"
INFISICAL_CLIENT_ID="6f4d8e91-3a2b-4c5d-9e7f-1a2b3c4d5e6f"  
INFISICAL_CLIENT_SECRET="<jeder braucht seinen eigenen Token>"
INFISICAL_ENVIRONMENT="dev"
```

Jedes Teammitglied:
1. Bekommt Zugang zum Infisical Projekt
2. Erstellt seinen eigenen Service Token
3. Kann sofort die Loxone Credentials nutzen

## Troubleshooting

**Problem: "Failed to create multi-backend credential manager"**
- Prüfe ob alle 3 INFISICAL_* Environment Variables gesetzt sind
- Prüfe ob der Service Token noch gültig ist

**Problem: "Credentials not found in Infisical"**
- Gehe ins Infisical Dashboard
- Stelle sicher dass LOXONE_USERNAME und LOXONE_PASSWORD als Secrets existieren
- Prüfe ob du das richtige Environment verwendest (dev/staging/prod)

## Vorteile

✅ Keine Passwörter im Code  
✅ Keine Keychain Popups  
✅ Team kann Credentials teilen  
✅ Funktioniert in CI/CD  
✅ Audit Log wer wann auf Secrets zugreift  
✅ Kostenlos für kleine Teams
# Schema del Database - Ruggine

## Panoramica dello Schema

Il database utilizza SQLite con un design ottimizzato per applicazioni di chat cross-platform. Lo schema supporta tutte le funzionalità richieste dalla traccia del progetto.

## Tabelle Principali

### 1. **users** - Gestione Utenti

```sql
CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,              -- UUID v4
    username TEXT UNIQUE NOT NULL,             -- Nome utente univoco
    created_at TEXT NOT NULL,                  -- Data registrazione (ISO 8601)
    last_seen TEXT,                           -- Ultimo accesso (ISO 8601)
    is_online BOOLEAN DEFAULT FALSE           -- Stato online
);
```

**Caratteristiche:**
- **Registrazione al primo avvio**: Soddisfa requisito traccia
- **Username univoci**: Controllo constraint UNIQUE
- **Tracking stato**: Online/offline con timestamp ultimo accesso
- **UUID**: Prevenzione enumeration attacks

**Esempio Dati:**
```sql
INSERT INTO users VALUES (
    '550e8400-e29b-41d4-a716-446655440000',
    'alice',
    '2024-01-15T10:30:00Z',
    '2024-01-15T15:45:00Z',
    true
);
```

### 2. **groups** - Gestione Gruppi

```sql
CREATE TABLE groups (
    id TEXT PRIMARY KEY NOT NULL,              -- UUID v4
    name TEXT NOT NULL,                        -- Nome gruppo
    description TEXT,                          -- Descrizione opzionale
    created_by TEXT NOT NULL,                  -- UUID creatore
    created_at TEXT NOT NULL,                  -- Data creazione
    is_active BOOLEAN DEFAULT TRUE,            -- Gruppo attivo
    max_members INTEGER DEFAULT 100,           -- Limite membri
    FOREIGN KEY (created_by) REFERENCES users(id)
);
```

**Caratteristiche:**
- **Gruppi di chat**: Soddisfa requisito traccia
- **Creatore**: Tracciamento chi ha creato il gruppo
- **Limite membri**: Controllo scalabilità
- **Soft delete**: is_active per disabilitazione

**Esempio Dati:**
```sql
INSERT INTO groups VALUES (
    '123e4567-e89b-12d3-a456-426614174000',
    'Sviluppatori Rust',
    'Gruppo per discussioni su Rust',
    '550e8400-e29b-41d4-a716-446655440000',
    '2024-01-15T11:00:00Z',
    true,
    50
);
```

### 3. **group_members** - Membri dei Gruppi

```sql
CREATE TABLE group_members (
    group_id TEXT NOT NULL,                    -- UUID gruppo
    user_id TEXT NOT NULL,                     -- UUID utente
    joined_at TEXT NOT NULL,                   -- Data ingresso
    role TEXT DEFAULT 'member',                -- Ruolo: admin/moderator/member
    PRIMARY KEY (group_id, user_id),
    FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
```

**Caratteristiche:**
- **Relazione many-to-many**: Utenti ↔ Gruppi
- **Sistema ruoli**: Admin, moderator, member
- **Cascade delete**: Pulizia automatica
- **Timestamp ingresso**: Tracking cronologia

**Esempio Dati:**
```sql
INSERT INTO group_members VALUES (
    '123e4567-e89b-12d3-a456-426614174000',
    '550e8400-e29b-41d4-a716-446655440000',
    '2024-01-15T11:00:00Z',
    'admin'
);
```

### 4. **messages** - Messaggi

```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,              -- UUID v4
    sender_id TEXT NOT NULL,                   -- UUID mittente
    group_id TEXT,                            -- UUID gruppo (NULL = messaggio diretto)
    receiver_id TEXT,                         -- UUID destinatario (messaggi diretti)
    content TEXT NOT NULL,                    -- Contenuto messaggio
    timestamp TEXT NOT NULL,                  -- Timestamp invio
    message_type TEXT NOT NULL DEFAULT 'text', -- Tipo: text/system/notification
    edited_at TEXT,                           -- Timestamp modifica
    is_deleted BOOLEAN DEFAULT FALSE,         -- Soft delete
    FOREIGN KEY (sender_id) REFERENCES users(id),
    FOREIGN KEY (group_id) REFERENCES groups(id),
    FOREIGN KEY (receiver_id) REFERENCES users(id)
);
```

**Caratteristiche:**
- **Messaggi gruppo + diretti**: group_id NULL = messaggio diretto
- **Tipologie multiple**: Testo, notifiche sistema, join/leave
- **Soft delete**: Messaggi "cancellati" mantenuti per audit
- **Modifiche**: Tracking timestamp edits

**Esempio Dati:**
```sql
-- Messaggio di gruppo
INSERT INTO messages VALUES (
    '789e0123-e45f-67g8-h901-234567890123',
    '550e8400-e29b-41d4-a716-446655440000',
    '123e4567-e89b-12d3-a456-426614174000',
    NULL,
    'Ciao a tutti! Come va lo sviluppo?',
    '2024-01-15T12:00:00Z',
    'text',
    NULL,
    false
);

-- Messaggio diretto
INSERT INTO messages VALUES (
    '456e7890-e12f-34g5-h678-901234567890',
    '550e8400-e29b-41d4-a716-446655440000',
    NULL,
    '667e8900-e29b-41d4-a716-446655440001',
    'Messaggio privato',
    '2024-01-15T12:05:00Z',
    'text',
    NULL,
    false
);
```

### 5. **group_invites** - Inviti ai Gruppi

```sql
CREATE TABLE group_invites (
    id TEXT PRIMARY KEY NOT NULL,              -- UUID v4
    group_id TEXT NOT NULL,                    -- UUID gruppo
    inviter_id TEXT NOT NULL,                  -- UUID chi invita
    invitee_id TEXT NOT NULL,                  -- UUID invitato
    created_at TEXT NOT NULL,                  -- Data invito
    expires_at TEXT,                          -- Scadenza invito
    status TEXT NOT NULL DEFAULT 'pending',   -- pending/accepted/rejected/expired
    responded_at TEXT,                        -- Data risposta
    FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
    FOREIGN KEY (inviter_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (invitee_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(group_id, invitee_id, status)      -- Previene inviti duplicati
);
```

**Caratteristiche:**
- **Sistema inviti**: Soddisfa requisito "ingresso su invito"
- **Stati multipli**: Pending, accepted, rejected, expired
- **Scadenza**: Inviti possono scadere automaticamente
- **Prevenzione duplicati**: Constraint su (gruppo, utente, stato)

**Esempio Dati:**
```sql
INSERT INTO group_invites VALUES (
    'abc12345-e678-90de-f123-456789abcdef',
    '123e4567-e89b-12d3-a456-426614174000',
    '550e8400-e29b-41d4-a716-446655440000',
    '667e8900-e29b-41d4-a716-446655440001',
    '2024-01-15T13:00:00Z',
    '2024-01-22T13:00:00Z',
    'pending',
    NULL
);
```

## Tabelle di Sistema

### 6. **performance_metrics** - Monitoraggio Performance

```sql
CREATE TABLE performance_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,      -- ID incrementale
    timestamp TEXT NOT NULL,                   -- Timestamp rilevazione
    cpu_usage_percent REAL NOT NULL,          -- Utilizzo CPU %
    memory_usage_mb REAL NOT NULL,            -- Utilizzo memoria MB
    active_connections INTEGER NOT NULL,       -- Connessioni attive
    messages_per_minute INTEGER NOT NULL,      -- Messaggi al minuto
    server_uptime_seconds INTEGER NOT NULL     -- Uptime server
);
```

**Caratteristiche:**
- **Requisito traccia**: "Log CPU ogni 2 minuti"
- **Metriche complete**: CPU, memoria, connessioni, throughput
- **Analisi performance**: Identificazione colli di bottiglia
- **Cleanup automatico**: Rimozione dati vecchi

### 7. **audit_log** - Log delle Operazioni

```sql
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,      -- ID incrementale
    timestamp TEXT NOT NULL,                   -- Timestamp operazione
    user_id TEXT,                             -- UUID utente (nullable)
    action TEXT NOT NULL,                     -- Azione eseguita
    resource_type TEXT NOT NULL,             -- Tipo risorsa: user/group/message
    resource_id TEXT,                         -- ID risorsa
    details TEXT,                            -- Dettagli JSON
    ip_address TEXT,                         -- IP address
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);
```

**Caratteristiche:**
- **Audit completo**: Tracciamento tutte le operazioni
- **Debug**: Identificazione problemi e errori
- **Sicurezza**: Monitoraggio accessi e modifiche
- **Compliance**: Log per analisi sicurezza

## Relazioni e Vincoli

### **Diagramma Relazioni**

```
users (1) ──── (N) group_members (N) ──── (1) groups
  │                                          │
  │                                          │
  │ (1)                                  (1) │
  │                                          │
  ├── (N) messages (sender)                  │
  │                                          │
  └── (N) messages (receiver)                │
                                             │
group_invites (N) ────────────────────── (1)┘
  │
  │ (N)
  │
users (inviter/invitee)
```

### **Vincoli di Integrità**

1. **Unicità Username**: `UNIQUE(username)` in users
2. **Prevenzione Inviti Duplicati**: `UNIQUE(group_id, invitee_id, status)`
3. **Cascade Delete**: Rimozione dipendenze automatica
4. **Foreign Key**: Integrità referenziale garantita
5. **Check Constraints**: Validazione stati e ruoli

### **Indici di Performance**

```sql
-- Query messaggi per gruppo (più frequente)
CREATE INDEX idx_messages_group_timestamp ON messages(group_id, timestamp DESC);

-- Query messaggi per mittente
CREATE INDEX idx_messages_sender_timestamp ON messages(sender_id, timestamp DESC);

-- Query messaggi diretti
CREATE INDEX idx_messages_receiver_timestamp ON messages(receiver_id, timestamp DESC);

-- Lookup membri gruppi
CREATE INDEX idx_group_members_user ON group_members(user_id);
CREATE INDEX idx_group_members_group ON group_members(group_id);

-- Inviti pendenti
CREATE INDEX idx_invites_invitee_status ON group_invites(invitee_id, status);

-- Performance monitoring
CREATE INDEX idx_performance_timestamp ON performance_metrics(timestamp DESC);

-- Audit log
CREATE INDEX idx_audit_timestamp ON audit_log(timestamp DESC);

-- Utenti
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_online ON users(is_online, last_seen DESC);
```

## Tipi di Dati

### **Convenzioni**

- **UUID**: Formato string UUID v4 (36 caratteri)
- **Timestamp**: ISO 8601 format (e.g., "2024-01-15T12:00:00Z")
- **Boolean**: SQLite INTEGER 0/1, mappato a Rust bool
- **Text**: UTF-8 strings, lunghezza variabile
- **Real**: Floating point per metriche numeriche

### **Validazioni**

- **Username**: Lunghezza 3-30 caratteri, alfanumerici + underscore
- **Group Name**: Lunghezza 1-50 caratteri
- **Message Content**: Massimo 4000 caratteri
- **UUID Format**: Regex validation UUID v4

## Migrazione e Versioning

### **File di Migrazione**

```
migrations/
├── 001_initial_schema.sql          ← Schema iniziale
├── 002_add_message_editing.sql     ← Funzionalità future
└── 003_performance_improvements.sql ← Ottimizzazioni future
```

### **Processo di Migrazione**

1. SQLx legge tutti i file `.sql` in ordine numerico
2. Applica solo migrazioni non ancora eseguite
3. Traccia versioni in tabella `_sqlx_migrations`
4. Rollback automatico in caso di errore

## Conformità Requisiti Traccia

| Requisito | Implementazione Schema | Stato |
|-----------|----------------------|-------|
| Chat con gruppi | Tabelle groups, group_members | ✅ |
| Inviti per ingresso gruppi | Tabella group_invites | ✅ |
| Registrazione primo avvio | Tabella users, constraint unique | ✅ |
| Cross-platform | SQLite schema universale | ✅ |
| Log CPU ogni 2 minuti | Tabella performance_metrics | ✅ |
| Ottimizzazione dimensioni | Schema minimale, indici essenziali | ✅ |

---

**Next**: [Indici e Performance](indexes.md) | [API Database](api.md)

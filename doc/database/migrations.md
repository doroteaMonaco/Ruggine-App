# Sistema di Migrazioni - Ruggine Database

## Panoramica

Il sistema di migrazioni utilizza SQLx per gestire l'evoluzione dello schema database in modo sicuro e versionato, garantendo compatibilità e rollback automatici.

## Struttura Migrazioni

### **Directory Layout**

```
ruggine/
├── migrations/
│   ├── 001_initial_schema.sql           ← Schema iniziale
│   ├── 002_add_message_editing.sql      ← Feature future
│   ├── 003_performance_improvements.sql ← Ottimizzazioni
│   └── ...
├── src/
│   └── server/
│       └── database.rs                  ← DatabaseManager
└── Cargo.toml
```

### **Convenzioni Naming**

- **Numerazione**: `001_`, `002_`, `003_` (ordinamento garantito)
- **Descrizione**: Nome descrittivo dell'operazione
- **Estensione**: `.sql` per file SQL puri

## Migrazione Iniziale

### **001_initial_schema.sql**

```sql
-- Tabella per gli utenti
CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT UNIQUE NOT NULL,
    created_at TEXT NOT NULL,
    last_seen TEXT,
    is_online BOOLEAN DEFAULT FALSE
);

-- Tabella per i gruppi
CREATE TABLE groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    is_active BOOLEAN DEFAULT TRUE,
    max_members INTEGER DEFAULT 100,
    FOREIGN KEY (created_by) REFERENCES users(id)
);

-- Tabella per i membri dei gruppi (relazione many-to-many)
CREATE TABLE group_members (
    group_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    joined_at TEXT NOT NULL,
    role TEXT DEFAULT 'member',
    PRIMARY KEY (group_id, user_id),
    FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Tabella per i messaggi
CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    sender_id TEXT NOT NULL,
    group_id TEXT,
    receiver_id TEXT,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    message_type TEXT NOT NULL DEFAULT 'text',
    edited_at TEXT,
    is_deleted BOOLEAN DEFAULT FALSE,
    FOREIGN KEY (sender_id) REFERENCES users(id),
    FOREIGN KEY (group_id) REFERENCES groups(id),
    FOREIGN KEY (receiver_id) REFERENCES users(id)
);

-- Tabella per gli inviti ai gruppi
CREATE TABLE group_invites (
    id TEXT PRIMARY KEY NOT NULL,
    group_id TEXT NOT NULL,
    inviter_id TEXT NOT NULL,
    invitee_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    responded_at TEXT,
    FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
    FOREIGN KEY (inviter_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (invitee_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(group_id, invitee_id, status)
);

-- Tabella per le metriche di performance (logging CPU ogni 2 minuti)
CREATE TABLE performance_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    cpu_usage_percent REAL NOT NULL,
    memory_usage_mb REAL NOT NULL,
    active_connections INTEGER NOT NULL,
    messages_per_minute INTEGER NOT NULL,
    server_uptime_seconds INTEGER NOT NULL
);

-- Tabella per l'audit log (tracciamento azioni sistema)
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    user_id TEXT,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    details TEXT,
    ip_address TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);

-- Indici per ottimizzare le query più frequenti
CREATE INDEX idx_messages_group_timestamp ON messages(group_id, timestamp DESC);
CREATE INDEX idx_messages_sender_timestamp ON messages(sender_id, timestamp DESC);
CREATE INDEX idx_messages_receiver_timestamp ON messages(receiver_id, timestamp DESC);
CREATE INDEX idx_group_members_user ON group_members(user_id);
CREATE INDEX idx_group_members_group ON group_members(group_id);
CREATE INDEX idx_invites_invitee_status ON group_invites(invitee_id, status);
CREATE INDEX idx_performance_timestamp ON performance_metrics(timestamp DESC);
CREATE INDEX idx_audit_timestamp ON audit_log(timestamp DESC);
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_online ON users(is_online, last_seen DESC);
```

## Processo di Migrazione

### **Setup Automatico**

```rust
// In DatabaseManager::new()
pub async fn new(database_url: &str) -> Result<Self> {
    info!("Connecting to database: {}", database_url);
    
    let pool = SqlitePool::connect(database_url).await?;
    
    // Applica migrazioni automaticamente
    sqlx::migrate!("./migrations").run(&pool).await?;
    
    info!("Database connection established and migrations applied");
    
    Ok(Self { pool })
}
```

### **Flusso Esecuzione**

1. **Connessione Database**: SQLx si connette a `sqlite:ruggine.db`
2. **Check Migrations Table**: Verifica esistenza `_sqlx_migrations`
3. **Scan Directory**: Legge tutti i file `.sql` in `migrations/`
4. **Ordinamento**: Ordina per nome file (numerazione)
5. **Applicazione**: Esegue solo migrazioni non applicate
6. **Tracking**: Registra migrazioni applicate in `_sqlx_migrations`

### **Tabella di Tracking**

SQLx crea automaticamente:

```sql
CREATE TABLE _sqlx_migrations (
    version TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time INTEGER NOT NULL
);
```

## Migrazioni Future

### **Esempio: 002_add_message_editing.sql**

```sql
-- Aggiunge funzionalità di editing messaggi

-- Aggiungi colonna per tracking edits
ALTER TABLE messages ADD COLUMN edit_count INTEGER DEFAULT 0;

-- Aggiungi tabella per cronologia edits
CREATE TABLE message_edits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id TEXT NOT NULL,
    old_content TEXT NOT NULL,
    new_content TEXT NOT NULL,
    edited_by TEXT NOT NULL,
    edited_at TEXT NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE,
    FOREIGN KEY (edited_by) REFERENCES users(id)
);

-- Indice per cronologia edits
CREATE INDEX idx_message_edits_message_id ON message_edits(message_id, edited_at DESC);

-- Trigger per incrementare contatore edits
CREATE TRIGGER increment_edit_count 
AFTER INSERT ON message_edits
BEGIN
    UPDATE messages 
    SET edit_count = edit_count + 1, edited_at = NEW.edited_at
    WHERE id = NEW.message_id;
END;
```

### **Esempio: 003_performance_improvements.sql**

```sql
-- Ottimizzazioni performance

-- Indici compositi avanzati
CREATE INDEX idx_messages_user_group_timestamp ON messages(sender_id, group_id, timestamp DESC);
CREATE INDEX idx_invites_group_status_created ON group_invites(group_id, status, created_at DESC);

-- View per query frequenti
CREATE VIEW user_stats AS
SELECT 
    u.id,
    u.username,
    COUNT(DISTINCT gm.group_id) as groups_count,
    COUNT(DISTINCT m.id) as messages_count,
    MAX(m.timestamp) as last_message_at
FROM users u
LEFT JOIN group_members gm ON u.id = gm.user_id
LEFT JOIN messages m ON u.id = m.sender_id
GROUP BY u.id, u.username;

-- Materialized view per dashboard (SQLite 3.38+)
CREATE TABLE dashboard_stats AS
SELECT 
    DATE(timestamp) as date,
    COUNT(*) as daily_messages,
    COUNT(DISTINCT sender_id) as active_users
FROM messages
GROUP BY DATE(timestamp);

-- Indice per dashboard
CREATE INDEX idx_dashboard_stats_date ON dashboard_stats(date DESC);
```

## Gestione Sicura

### **Rollback Automatico**

```rust
// SQLx gestisce rollback automatico in caso di errore
async fn apply_migration() -> Result<()> {
    let mut tx = pool.begin().await?;
    
    // Se qualunque statement fallisce, rollback automatico
    sqlx::query("CREATE TABLE ...").execute(&mut tx).await?;
    sqlx::query("CREATE INDEX ...").execute(&mut tx).await?;
    
    tx.commit().await?; // Solo se tutto ha successo
    Ok(())
}
```

### **Validazione Schema**

```rust
// Check integrità post-migrazione
async fn validate_schema(pool: &SqlitePool) -> Result<()> {
    // Verifica tabelle esistenti
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    ).fetch_all(pool).await?;
    
    let expected = vec!["users", "groups", "group_members", "messages", 
                       "group_invites", "performance_metrics", "audit_log"];
    
    for table in expected {
        if !tables.contains(&table.to_string()) {
            return Err(anyhow::anyhow!("Missing table: {}", table));
        }
    }
    
    Ok(())
}
```

## Testing Migrazioni

### **Test Automatici**

```rust
#[cfg(test)]
mod migration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fresh_migration() -> Result<()> {
        // Database temporaneo
        let db_manager = DatabaseManager::new(":memory:").await?;
        
        // Verifica schema completo
        validate_schema(&db_manager.pool).await?;
        
        // Test operazioni base
        let user = User::new("test_user");
        db_manager.create_user(&user).await?;
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_incremental_migration() -> Result<()> {
        // Simula migrazione da versione precedente
        let temp_db = tempfile::NamedTempFile::new()?;
        
        // Applica solo prima migrazione
        apply_migration_subset(&temp_db, 1).await?;
        
        // Applica rimanenti
        let db_manager = DatabaseManager::new(
            &format!("sqlite:{}", temp_db.path().display())
        ).await?;
        
        // Verifica schema finale
        validate_schema(&db_manager.pool).await?;
        
        Ok(())
    }
}
```

### **Backup Pre-Migrazione**

```rust
async fn backup_before_migration(db_path: &str) -> Result<String> {
    let backup_path = format!("{}.backup.{}", db_path, 
        chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    
    std::fs::copy(db_path, &backup_path)?;
    
    info!("Database backed up to: {}", backup_path);
    Ok(backup_path)
}
```

## Best Practices

### **✅ Scrittura Migrazioni**

1. **Atomiche**: Ogni migrazione deve essere completamente reversibile
2. **Idempotenti**: Eseguibili multiple volte senza errori
3. **Testate**: Validate su database reali prima del deploy
4. **Documentate**: Commenti chiari su purpose e impatto

### **✅ Gestione Versioning**

1. **Numerazione Sequenziale**: 001, 002, 003...
2. **Nomi Descrittivi**: `add_user_preferences`, `optimize_indexes`
3. **Mai Modificare**: Migrazioni applicate non vanno mai cambiate
4. **Backward Compatibility**: Mantenere compatibilità API

### **❌ Evitare**

1. **Breaking Changes**: Modifiche che rompono codice esistente
2. **Data Loss**: Operazioni che possono perdere dati
3. **Modifiche Post-Deploy**: Cambiare migrazioni già applicate
4. **Dipendenze Esterne**: Migrazioni che dipendono da dati esterni

## Troubleshooting

### **Errori Comuni**

```bash
# Migrazione fallita
ERROR: migration 002 failed: syntax error near "ALTERABLE"

# Soluzione: Verifica sintassi SQL
```

```bash
# Checksum mismatch
ERROR: migration 001 checksum mismatch

# Soluzione: Non modificare migrazioni già applicate
```

### **Recovery**

```rust
// Reset migrazioni (SOLO in sviluppo)
async fn reset_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::query("DROP TABLE IF EXISTS _sqlx_migrations").execute(pool).await?;
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
```

### **Verifica Stato**

```sql
-- Controlla migrazioni applicate
SELECT version, description, installed_on, success 
FROM _sqlx_migrations 
ORDER BY installed_on;

-- Verifica integrità schema
PRAGMA integrity_check;

-- Analizza performance
PRAGMA optimize;
```

## Deployment Production

### **Setup CI/CD**

```yaml
# .github/workflows/deploy.yml
- name: Run database migrations
  run: |
    # Backup automatico
    cp production.db production.db.backup.$(date +%Y%m%d_%H%M%S)
    
    # Applica migrazioni
    cargo run --bin migrate
    
    # Verifica integrità
    sqlite3 production.db "PRAGMA integrity_check;"
```

### **Monitoring Migrazioni**

```rust
// Log dettagliato migrazioni
async fn log_migration_status(pool: &SqlitePool) -> Result<()> {
    let migrations = sqlx::query_as::<_, Migration>(
        "SELECT * FROM _sqlx_migrations ORDER BY installed_on DESC LIMIT 5"
    ).fetch_all(pool).await?;
    
    for migration in migrations {
        info!("Applied: {} - {} at {}", 
              migration.version, migration.description, migration.installed_on);
    }
    
    Ok(())
}
```

---

**Next**: [Performance Monitoring](monitoring.md) | [Deployment](deployment.md)

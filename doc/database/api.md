# API Database - Ruggine

## DatabaseManager API

Il `DatabaseManager` fornisce un'interfaccia Rust async per tutte le operazioni database, implementando il pattern Repository per separare la logica di business dalla persistenza.

## Architettura API

```rust
pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    // Connessione e setup
    pub async fn new(database_url: &str) -> Result<Self>
    
    // Operazioni utenti
    pub async fn create_user(&self, user: &User) -> Result<()>
    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>>
    pub async fn update_user_online_status(&self, user_id: Uuid, is_online: bool) -> Result<()>
    pub async fn get_online_users(&self) -> Result<Vec<User>>
    pub async fn get_all_users(&self) -> Result<Vec<User>>
    
    // Operazioni gruppi
    pub async fn create_group(&self, group: &Group) -> Result<()>
    pub async fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<Group>>
    pub async fn get_group_members(&self, group_id: Uuid) -> Result<Vec<Uuid>>
    
    // Operazioni messaggi
    pub async fn save_message(&self, message: &Message) -> Result<()>
    pub async fn save_direct_message(&self, sender_id: Uuid, receiver_id: Uuid, content: &str, message_type: MessageType) -> Result<Uuid>
    pub async fn get_group_messages(&self, group_id: Uuid, limit: i64) -> Result<Vec<Message>>
    pub async fn get_direct_messages(&self, user1_id: Uuid, user2_id: Uuid, limit: i64) -> Result<Vec<Message>>
    
    // Operazioni inviti
    pub async fn create_group_invite(&self, invite: &GroupInvite) -> Result<()>
    pub async fn get_pending_invites(&self, user_id: Uuid) -> Result<Vec<GroupInvite>>
    pub async fn accept_group_invite(&self, invite_id: Uuid) -> Result<()>
    
    // Performance monitoring (requisito traccia)
    pub async fn save_performance_metrics(&self, metrics: &PerformanceMetrics) -> Result<()>
    
    // Audit e logging
    pub async fn log_action(&self, user_id: Option<Uuid>, action: &str, resource_type: &str, resource_id: Option<&str>, details: Option<&str>, ip_address: Option<&str>) -> Result<()>
    
    // Utilità
    pub async fn get_database_stats(&self) -> Result<(usize, usize, usize, usize)>
    pub async fn cleanup_old_data(&self, days_to_keep: i64) -> Result<()>
}
```

## Gestione Utenti

### **Registrazione Utente (Primo Avvio)**

```rust
pub async fn create_user(&self, user: &User) -> Result<()>
```

**Funzionalità:**
- Crea nuovo utente con UUID v4
- Controllo unicità username (constraint DB)
- Timestamp registrazione automatico
- Stato online inizializzato

**Esempio Utilizzo:**
```rust
let user = User {
    id: Uuid::new_v4(),
    username: "alice".to_string(),
    created_at: Utc::now(),
    is_online: true,
};

db_manager.create_user(&user).await?;
```

**Errori Gestiti:**
- `UNIQUE constraint failed: users.username` → Username già esistente
- Validazione formato UUID
- Validazione lunghezza username

### **Autenticazione/Lookup**

```rust
pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>>
```

**Funzionalità:**
- Ricerca veloce per username (indice ottimizzato)
- Ritorna `Option<User>` per gestione sicura
- Include stato online e ultimo accesso

**Esempio Utilizzo:**
```rust
match db_manager.get_user_by_username("alice").await? {
    Some(user) => println!("User found: {}", user.id),
    None => println!("User not found"),
}
```

### **Gestione Stato Online**

```rust
pub async fn update_user_online_status(&self, user_id: Uuid, is_online: bool) -> Result<()>
```

**Funzionalità:**
- Aggiorna stato online/offline
- Timestamp automatico ultimo accesso se offline
- Ottimizzato per chiamate frequenti

**Esempio Utilizzo:**
```rust
// Utente si connette
db_manager.update_user_online_status(user_id, true).await?;

// Utente si disconnette
db_manager.update_user_online_status(user_id, false).await?;
```

## Gestione Gruppi

### **Creazione Gruppo**

```rust
pub async fn create_group(&self, group: &Group) -> Result<()>
```

**Funzionalità:**
- Crea gruppo con creatore come admin
- Transazione atomica: gruppo + membership
- Controllo duplicati nomi gruppo

**Esempio Utilizzo:**
```rust
let group = Group {
    id: Uuid::new_v4(),
    name: "Sviluppatori Rust".to_string(),
    description: Some("Discussioni tecniche".to_string()),
    created_by: creator_id,
    created_at: Utc::now(),
    members: vec![creator_id],
};

db_manager.create_group(&group).await?;
```

### **Query Gruppi Utente**

```rust
pub async fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<Group>>
```

**Funzionalità:**
- Lista tutti i gruppi di cui fa parte l'utente
- Include informazioni complete gruppo
- Popolamento membri automatico

**Esempio Utilizzo:**
```rust
let user_groups = db_manager.get_user_groups(user_id).await?;
for group in user_groups {
    println!("Gruppo: {} ({} membri)", group.name, group.members.len());
}
```

### **Gestione Membri**

```rust
pub async fn get_group_members(&self, group_id: Uuid) -> Result<Vec<Uuid>>
```

**Funzionalità:**
- Lista UUID di tutti i membri
- Utilizzabile per controlli permessi
- Performance ottimizzata con indice

## Gestione Messaggi

### **Messaggi di Gruppo**

```rust
pub async fn save_message(&self, message: &Message) -> Result<()>
pub async fn get_group_messages(&self, group_id: Uuid, limit: i64) -> Result<Vec<Message>>
```

**Funzionalità save_message:**
- Salvataggio con timestamp automatico
- Supporto diverse tipologie messaggio
- Gestione messaggi gruppo e diretti

**Funzionalità get_group_messages:**
- Recupero messaggi recenti ordinati
- Esclusione messaggi cancellati
- Limit per paginazione

**Esempio Utilizzo:**
```rust
// Salva messaggio
let message = Message {
    id: Uuid::new_v4(),
    sender_id,
    group_id: Some(group_id),
    content: "Ciao gruppo!".to_string(),
    timestamp: Utc::now(),
    message_type: MessageType::Text,
};

db_manager.save_message(&message).await?;

// Recupera ultimi 50 messaggi
let messages = db_manager.get_group_messages(group_id, 50).await?;
```

### **Messaggi Diretti**

```rust
pub async fn save_direct_message(&self, sender_id: Uuid, receiver_id: Uuid, content: &str, message_type: MessageType) -> Result<Uuid>
pub async fn get_direct_messages(&self, user1_id: Uuid, user2_id: Uuid, limit: i64) -> Result<Vec<Message>>
```

**Funzionalità:**
- Messaggi privati tra due utenti
- Query bidirezionale (A→B e B→A)
- Cronologia completa conversazione

**Esempio Utilizzo:**
```rust
// Invia messaggio diretto
let msg_id = db_manager.save_direct_message(
    sender_id,
    receiver_id,
    "Messaggio privato",
    MessageType::Text
).await?;

// Recupera conversazione
let conversation = db_manager.get_direct_messages(user1_id, user2_id, 100).await?;
```

## Sistema Inviti

### **Creazione Invito**

```rust
pub async fn create_group_invite(&self, invite: &GroupInvite) -> Result<()>
```

**Funzionalità:**
- Invito con scadenza opzionale
- Prevenzione inviti duplicati (constraint DB)
- Stato iniziale: Pending

**Esempio Utilizzo:**
```rust
let invite = GroupInvite {
    id: Uuid::new_v4(),
    group_id,
    inviter_id,
    invitee_id,
    created_at: Utc::now(),
    status: InviteStatus::Pending,
};

db_manager.create_group_invite(&invite).await?;
```

### **Gestione Inviti**

```rust
pub async fn get_pending_invites(&self, user_id: Uuid) -> Result<Vec<GroupInvite>>
pub async fn accept_group_invite(&self, invite_id: Uuid) -> Result<()>
```

**Funzionalità get_pending_invites:**
- Lista inviti in stato Pending
- Ordinamento cronologico
- Filtraggio per destinatario

**Funzionalità accept_group_invite:**
- Transazione atomica: aggiorna invito + aggiungi membro
- Controllo stato e permessi
- Prevenzione duplicati membership

## Performance Monitoring (Requisito Traccia)

### **Logging Metriche CPU**

```rust
pub async fn save_performance_metrics(&self, metrics: &PerformanceMetrics) -> Result<()>
```

**Funzionalità:**
- Salvataggio metriche ogni 2 minuti (requisito traccia)
- CPU usage, memoria, connessioni attive
- Throughput messaggi per minuto

**Esempio Utilizzo:**
```rust
let metrics = PerformanceMetrics {
    timestamp: Utc::now(),
    cpu_usage_percent: 15.5,
    memory_usage_mb: 125.0,
    active_connections: 42,
    messages_per_minute: 150,
};

db_manager.save_performance_metrics(&metrics).await?;
```

### **Cleanup Automatico**

```rust
pub async fn cleanup_old_data(&self, days_to_keep: i64) -> Result<()>
```

**Funzionalità:**
- Rimozione metriche vecchie
- Pulizia inviti scaduti/rifiutati
- Mantenimento performance database

## Audit e Logging

### **Log Operazioni**

```rust
pub async fn log_action(&self, user_id: Option<Uuid>, action: &str, resource_type: &str, resource_id: Option<&str>, details: Option<&str>, ip_address: Option<&str>) -> Result<()>
```

**Funzionalità:**
- Tracciamento completo operazioni
- Support operazioni sistema (user_id None)
- Dettagli JSON per informazioni aggiuntive

**Esempio Utilizzo:**
```rust
// Log creazione gruppo
db_manager.log_action(
    Some(user_id),
    "CREATE",
    "group",
    Some(&group_id.to_string()),
    Some(&format!("{{\"name\": \"{}\"}}", group_name)),
    Some("192.168.1.100")
).await?;

// Log sistema
db_manager.log_action(
    None,
    "CLEANUP",
    "system",
    None,
    Some("{\"removed_metrics\": 1500}"),
    None
).await?;
```

## Statistiche e Utilità

### **Statistiche Database**

```rust
pub async fn get_database_stats(&self) -> Result<(usize, usize, usize, usize)>
```

**Funzionalità:**
- Conteggi totali: utenti, gruppi, messaggi, inviti pendenti
- Performance monitoring dashboard
- Utilizzo in metriche sistema

**Esempio Utilizzo:**
```rust
let (users_count, groups_count, messages_count, pending_invites) = 
    db_manager.get_database_stats().await?;

println!("Stats: {} users, {} groups, {} messages, {} pending invites",
    users_count, groups_count, messages_count, pending_invites);
```

### **Lista Utenti Online**

```rust
pub async fn get_online_users(&self) -> Result<Vec<User>>
```

**Funzionalità:**
- Filtraggio utenti con is_online = true
- Ordinamento per ultimo accesso
- Dashboard presenza utenti

## Error Handling

### **Tipi di Errore**

```rust
use anyhow::Result;

// Errori comuni gestiti
pub enum DatabaseError {
    ConnectionFailed,
    UniqueConstraintViolation,
    ForeignKeyConstraintViolation,
    NotFound,
    SerializationError,
    TransactionFailed,
}
```

### **Gestione Robusta**

```rust
// Pattern error handling raccomandato
match db_manager.create_user(&user).await {
    Ok(()) => println!("User created successfully"),
    Err(e) if e.to_string().contains("UNIQUE constraint") => {
        println!("Username already taken");
    },
    Err(e) => {
        error!("Database error: {}", e);
        return Err(e);
    }
}
```

## Transazioni

### **Operazioni Atomiche**

Operazioni che richiedono transazioni sono gestite automaticamente:

- **Creazione gruppo**: gruppo + membership iniziale
- **Accettazione invito**: update invito + add membership
- **Cleanup**: multiple delete operations

```rust
// Implementazione interna con transazioni
pub async fn create_group(&self, group: &Group) -> Result<()> {
    let mut tx = self.pool.begin().await?;
    
    // Insert gruppo
    sqlx::query("INSERT INTO groups ...").execute(&mut *tx).await?;
    
    // Insert creatore come admin
    sqlx::query("INSERT INTO group_members ...").execute(&mut *tx).await?;
    
    tx.commit().await?;
    Ok(())
}
```

## Configurazione e Setup

### **Inizializzazione**

```rust
// Setup automatico con migrazioni
let db_manager = DatabaseManager::new("sqlite:ruggine.db").await?;

// Database pronto all'uso - nessuna configurazione aggiuntiva richiesta
```

### **Connection Pool**

```rust
// Configurazione ottimizzata automatica
SqlitePoolOptions::new()
    .max_connections(20)
    .connect_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(600))
```

---

**Next**: [Migrazioni](migrations.md) | [Performance Monitoring](monitoring.md)

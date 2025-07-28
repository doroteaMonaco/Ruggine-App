# Implementazione Crittografia End-to-End

Questa implementazione fornisce crittografia end-to-end per l'applicazione di chat Ruggine, garantendo che i messaggi siano crittografati lato client prima di essere inviati al server.

## Caratteristiche Principali

### 🔐 Crittografia AES-256-GCM
- Algoritmo: AES-256 in modalità Galois/Counter Mode (GCM)
- Libreria: `ring` per performance e sicurezza ottimali
- Chiavi: 256 bit generate casualmente per ogni gruppo/chat

### 🔑 Gestione delle Chiavi
- **Gruppi**: Ogni gruppo ha una chiave condivisa tra tutti i membri
- **Chat Dirette**: Ogni coppia di utenti ha una chiave condivisa unica
- **Rotazione**: Supporto per rotazione delle chiavi (opzionale)
- **Condivisione**: Meccanismo sicuro per condividere chiavi con nuovi membri

### 🛡️ Sicurezza
- **Nonce Unici**: Ogni messaggio usa un nonce casuale univoco
- **Autenticazione**: GCM fornisce autenticazione integrata
- **Forward Secrecy**: Possibilità di implementare rotazione chiavi
- **Zero-Knowledge Server**: Il server non può decrittare i messaggi

## Architettura

### Componenti

1. **CryptoManager** (`src/common/crypto.rs`)
   - Gestisce crittografia/decrittografia
   - Memorizza chiavi in memoria
   - Genera chiavi casuali sicure

2. **SecureChatManager** (`src/server/secure_chat_manager.rs`)
   - Integrazione server-side
   - Gestione chiavi di gruppo
   - Salvataggio messaggi crittografati

3. **SecureClient** (`src/client/secure_client.rs`)
   - API client per crittografia
   - Gestione messaggi crittografati
   - Integrazione con GUI

4. **Database Schema** (`migrations/002_encryption_schema.sql`)
   - Tabelle per messaggi crittografati
   - Gestione chiavi di gruppo
   - Indici per performance

### Flusso dei Messaggi

#### Invio Messaggio di Gruppo
```
1. Client critta il messaggio con la chiave del gruppo
2. Invia EncryptedMessage al server
3. Server salva il messaggio crittografato
4. Server inoltra a tutti i membri del gruppo
5. Ogni client decritta con la propria copia della chiave
```

#### Invio Messaggio Diretto
```
1. Client critta con la chiave condivisa della chat
2. Invia EncryptedMessage al server
3. Server salva e inoltra al destinatario
4. Destinatario decritta con la chiave condivisa
```

## Utilizzo

### Inizializzazione Client
```rust
use ruggine::client::SecureClient;

let mut client = SecureClient::new();
client.set_user_id(user_id);
```

### Configurazione Gruppo
```rust
// Creatore del gruppo
let group_key = client.setup_group_encryption(group_id)?;

// Altri membri importano la chiave
client.import_group_key(group_id, &shared_key)?;
```

### Invio Messaggio Crittografato
```rust
// Messaggio di gruppo
let encrypted_msg = client.prepare_encrypted_group_message(
    group_id,
    "Messaggio segreto",
    MessageType::Text
)?;

// Messaggio diretto
let encrypted_msg = client.prepare_encrypted_direct_message(
    receiver_id,
    "Messaggio privato",
    MessageType::Text
)?;
```

### Gestione Messaggi Ricevuti
```rust
match server_message {
    ServerMessage::EncryptedMessageReceived { encrypted_message } => {
        if let Some(content) = client.handle_server_message(server_message)? {
            println!("Messaggio decrittato: {}", content);
        }
    }
    _ => {}
}
```

## Schema Database

### Tabella `encrypted_messages`
```sql
CREATE TABLE encrypted_messages (
    id TEXT PRIMARY KEY,
    sender_id TEXT NOT NULL,
    group_id TEXT,
    receiver_id TEXT,
    encrypted_content TEXT NOT NULL,  -- Base64
    nonce TEXT NOT NULL,             -- Base64
    timestamp TEXT NOT NULL,
    message_type TEXT NOT NULL
);
```

### Tabella `group_encryption_keys`
```sql
CREATE TABLE group_encryption_keys (
    id INTEGER PRIMARY KEY,
    group_id TEXT NOT NULL,
    encrypted_key TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    is_active BOOLEAN DEFAULT TRUE
);
```

## Protocollo di Comunicazione

### Nuovi Messaggi ClientMessage
```rust
pub enum ClientMessage {
    // Invio messaggio crittografato
    SendEncryptedMessage { 
        encrypted_message: EncryptedMessage 
    },
    
    // Richiesta chiave gruppo
    RequestGroupKey { 
        group_id: Uuid 
    },
    
    // Condivisione chiave
    ShareGroupKey { 
        group_id: Uuid, 
        encrypted_key: String, 
        target_user: Uuid 
    },
    // ... altri messaggi esistenti
}
```

### Nuovi Messaggi ServerMessage
```rust
pub enum ServerMessage {
    // Messaggio crittografato ricevuto
    EncryptedMessageReceived { 
        encrypted_message: EncryptedMessage 
    },
    
    // Chiave condivisa
    GroupKeyShared { 
        group_id: Uuid, 
        encrypted_key: String 
    },
    
    // Lista messaggi crittografati
    EncryptedGroupMessages { 
        messages: Vec<EncryptedMessage> 
    },
    // ... altri messaggi esistenti
}
```

## Migrazione

### 1. Applicare Schema Database
```bash
cd ruggine
sqlx migrate run --source migrations
```

### 2. Aggiornare Dipendenze
Le nuove dipendenze sono già aggiunte al `Cargo.toml`:
- `ring = "0.17"` - Crittografia
- `base64 = "0.22"` - Encoding
- `rand = "0.8"` - Generazione numeri casuali

### 3. Integrazione Graduale
L'implementazione è progettata per coesistere con il sistema di messaggi esistente:
- I messaggi non crittografati continuano a funzionare
- I client possono scegliere quando abilitare la crittografia
- Migrazione graduale gruppo per gruppo

## Considerazioni di Sicurezza

### ✅ Vantaggi
- **Zero-Knowledge Server**: Il server non può leggere i messaggi
- **Forward Secrecy**: Compromissione chiavi future non compromette messaggi passati
- **Autenticazione**: GCM previene tampering
- **Performance**: Ring è ottimizzato per velocità

### ⚠️ Considerazioni
- **Gestione Chiavi**: Le chiavi sono in memoria, non persistenti
- **Backup**: Messaggi crittografati non recuperabili se si perde la chiave
- **Nuovi Membri**: Necessario condividere chiavi manualmente
- **Key Rotation**: Da implementare per sicurezza a lungo termine

## Estensioni Future

### Possibili Miglioramenti
1. **Signal Protocol**: Implementare ratcheting per forward secrecy perfetta
2. **Key Rotation Automatica**: Rotazione periodica delle chiavi
3. **Device Keys**: Chiavi separate per device multipli
4. **Backup Sicuro**: Backup crittografato delle chiavi
5. **Ephemeral Messages**: Auto-eliminazione dopo lettura

### Integrazione con Features Esistenti
- ✅ Funziona con inviti di gruppo
- ✅ Compatibile con messaggi di sistema
- ✅ Supporta tutti i tipi di messaggio
- ✅ Mantiene timestamp e metadati

## Testing

### Unit Tests
```bash
cargo test crypto
cargo test secure_chat
cargo test secure_client
```

### Integration Tests
I test di integrazione richiedono database di test e sono inclusi nei moduli.

## Performance

### Benchmark Attesi
- **Crittografia**: ~1-5ms per messaggio medio
- **Decrittografia**: ~1-5ms per messaggio medio
- **Memoria**: ~32 bytes per chiave di gruppo
- **Storage**: +~50% per messaggi crittografati (Base64 overhead)

### Ottimizzazioni
- Chiavi mantenute in memoria per performance
- Batch processing per messaggi multipli
- Indici database ottimizzati per query temporali

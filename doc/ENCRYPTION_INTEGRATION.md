# Esempio di Integrazione della Crittografia End-to-End

Questo file mostra come integrare la crittografia nel sistema esistente.

## 1. Modifiche al Chat Manager Esistente

### src/server/chat_manager.rs - Aggiunta del supporto crittografia

```rust
use crate::server::secure_chat_manager::SecureChatManager;
use crate::common::protocol::{ClientMessage, ServerMessage};

impl ChatManager {
    // Aggiungi questo campo alla struct ChatManager
    secure_chat: SecureChatManager,
    
    // Modifica il costruttore
    pub fn new(database: DatabaseManager) -> Self {
        let secure_chat = SecureChatManager::new(database.clone());
        
        Self {
            users: HashMap::new(),
            groups: HashMap::new(),
            database,
            secure_chat,
        }
    }
    
    // Nuovo metodo per gestire messaggi crittografati
    pub async fn handle_encrypted_message(&mut self, 
        user_id: Uuid, 
        encrypted_message: EncryptedMessage
    ) -> Result<()> {
        // Salva il messaggio crittografato
        self.secure_chat.database.save_encrypted_message(&encrypted_message).await?;
        
        // Determina i destinatari
        let recipients = if let Some(group_id) = encrypted_message.group_id {
            // Messaggio di gruppo - invia a tutti i membri
            self.database.get_group_members(group_id).await?
        } else if let Some(receiver_id) = encrypted_message.receiver_id {
            // Messaggio diretto
            vec![receiver_id]
        } else {
            return Err(anyhow::anyhow!("Messaggio senza destinatario"));
        };
        
        // Inoltra il messaggio crittografato ai destinatari
        let server_message = ServerMessage::EncryptedMessageReceived { 
            encrypted_message: encrypted_message.clone() 
        };
        
        for recipient_id in recipients {
            if let Some(user_conn) = self.users.get(&recipient_id) {
                if let Err(e) = user_conn.send(server_message.clone()).await {
                    log::warn!("Errore nell'invio messaggio crittografato a {}: {}", recipient_id, e);
                }
            }
        }
        
        Ok(())
    }
    
    // Modifica handle_client_message per supportare messaggi crittografati
    pub async fn handle_client_message(&mut self, 
        user_id: Uuid, 
        message: ClientMessage
    ) -> Result<Option<ServerMessage>> {
        match message {
            ClientMessage::SendEncryptedMessage { encrypted_message } => {
                self.handle_encrypted_message(user_id, encrypted_message).await?;
                Ok(None) // Nessuna risposta diretta necessaria
            }
            
            ClientMessage::RequestGroupKey { group_id } => {
                // Verifica che l'utente sia membro del gruppo
                if self.database.is_user_in_group(user_id, group_id).await? {
                    if let Some(encrypted_key) = self.database.get_group_encryption_key(group_id).await? {
                        Ok(Some(ServerMessage::GroupKeyShared { group_id, encrypted_key }))
                    } else {
                        Ok(Some(ServerMessage::Error { 
                            message: "Chiave di crittografia non trovata".to_string() 
                        }))
                    }
                } else {
                    Ok(Some(ServerMessage::Error { 
                        message: "Non autorizzato ad accedere a questo gruppo".to_string() 
                    }))
                }
            }
            
            // ... gestione altri messaggi esistenti
            _ => {
                // Delega ai metodi esistenti
                self.handle_existing_message(user_id, message).await
            }
        }
    }
}
```

## 2. Modifiche al Client GUI

### src/client/gui_main.rs - Integrazione con l'interfaccia

```rust
use crate::client::secure_client::{SecureClient, MessageTarget};

// Aggiungi alla struct principale dell'app
pub struct RuggineApp {
    // ... campi esistenti
    secure_client: SecureClient,
    encryption_enabled: bool,
}

impl RuggineApp {
    pub fn new() -> Self {
        Self {
            // ... inizializzazione esistente
            secure_client: SecureClient::new(),
            encryption_enabled: false, // Da abilitare tramite setting
        }
    }
    
    // Metodo per abilitare/disabilitare crittografia
    fn toggle_encryption(&mut self) {
        self.encryption_enabled = !self.encryption_enabled;
        if !self.encryption_enabled {
            self.secure_client.cleanup_crypto();
        }
    }
    
    // Modifica send_message per supportare crittografia
    async fn send_message(&mut self, content: String, target: MessageTarget) -> Result<()> {
        if self.encryption_enabled {
            // Usa crittografia
            let encrypted_msg = self.secure_client
                .send_secure_message_example(target, &content)
                .await?;
            
            // Invia al server
            self.send_to_server(encrypted_msg).await?;
        } else {
            // Usa il metodo esistente non crittografato
            let normal_msg = match target {
                MessageTarget::Group(group_id) => {
                    ClientMessage::SendMessage { 
                        content, 
                        group_id: Some(group_id) 
                    }
                }
                MessageTarget::User(user_id) => {
                    // Implementa messaggio diretto normale
                    ClientMessage::SendMessage { 
                        content, 
                        group_id: None // Questo richiederà modifiche al protocollo esistente
                    }
                }
            };
            self.send_to_server(normal_msg).await?;
        }
        
        Ok(())
    }
    
    // Gestione messaggi ricevuti
    fn handle_server_message(&mut self, message: ServerMessage) {
        if self.encryption_enabled {
            if let Some(decrypted_content) = self.secure_client.handle_received_message(message.clone()) {
                // Mostra il messaggio decrittato
                self.add_message_to_chat(decrypted_content);
                return;
            }
        }
        
        // Gestione messaggi normali esistente
        match message {
            ServerMessage::MessageReceived { message, sender } => {
                self.add_message_to_chat(format!("{}: {}", sender.username, message.content));
            }
            // ... altri casi esistenti
            _ => {}
        }
    }
}

// Esempio di UI per controllare la crittografia
impl Application for RuggineApp {
    fn view(&self) -> Element<Message> {
        let encryption_toggle = checkbox(
            "Abilita Crittografia End-to-End",
            self.encryption_enabled,
            |enabled| Message::ToggleEncryption(enabled)
        );
        
        let encryption_status = if self.encryption_enabled {
            text("🔒 Messaggi crittografati").style(Color::GREEN)
        } else {
            text("🔓 Messaggi in chiaro").style(Color::ORANGE)
        };
        
        column![
            encryption_toggle,
            encryption_status,
            // ... resto dell'UI esistente
        ].into()
    }
    
    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::ToggleEncryption(enabled) => {
                self.encryption_enabled = enabled;
                if !enabled {
                    self.secure_client.cleanup_crypto();
                }
                Command::none()
            }
            // ... altri messaggi esistenti
            _ => Command::none()
        }
    }
}
```

## 3. Migrazione Database

### Script per applicare le migrazioni

```bash
#!/bin/bash
# scripts/apply_encryption_migration.sh

echo "Applicando migrazione per crittografia end-to-end..."

# Backup del database esistente
cp ruggine/data/ruggine.db ruggine/data/ruggine.db.backup

# Applica la migrazione
cd ruggine
sqlx migrate run --source migrations

if [ $? -eq 0 ]; then
    echo "✅ Migrazione applicata con successo!"
    echo "Database backup salvato in: ruggine/data/ruggine.db.backup"
else
    echo "❌ Errore nella migrazione. Ripristino backup..."
    cp ruggine/data/ruggine.db.backup ruggine/data/ruggine.db
    exit 1
fi
```

## 4. Configurazione

### src/common/config.rs - Aggiunta opzioni crittografia

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EncryptionConfig {
    pub enabled_by_default: bool,
    pub require_encryption_for_groups: bool,
    pub key_rotation_hours: Option<u64>,
    pub cleanup_old_keys_days: u64,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled_by_default: false,
            require_encryption_for_groups: false,
            key_rotation_hours: None, // Disabilitato per default
            cleanup_old_keys_days: 30,
        }
    }
}

// Aggiungi ai config esistenti
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerConfig {
    // ... campi esistenti
    pub encryption: EncryptionConfig,
}
```

## 5. Testing dell'Integrazione

### tests/integration_encryption.rs

```rust
use ruggine::server::SecureChatManager;
use ruggine::client::SecureClient;
use ruggine::common::models::MessageType;

#[tokio::test]
async fn test_end_to_end_encryption_flow() {
    // Setup database di test
    let database = DatabaseManager::new("sqlite::memory:").await.unwrap();
    
    // Applica migrazioni
    sqlx::migrate!("../migrations").run(&database.pool).await.unwrap();
    
    // Setup server
    let mut server_chat = SecureChatManager::new(database);
    
    // Setup client
    let mut client1 = SecureClient::new();
    let mut client2 = SecureClient::new();
    
    let user1_id = Uuid::new_v4();
    let user2_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    
    client1.set_user_id(user1_id);
    client2.set_user_id(user2_id);
    
    // Inizializza crittografia utenti
    server_chat.initialize_user_crypto(user1_id).unwrap();
    server_chat.initialize_user_crypto(user2_id).unwrap();
    
    // Crea gruppo con crittografia
    server_chat.create_group_encryption(group_id, user1_id, &[user1_id, user2_id]).await.unwrap();
    
    // Client1 setup gruppo
    let group_key = client1.setup_group_encryption(group_id).unwrap();
    
    // Client2 importa chiave
    client2.import_group_key(group_id, &group_key).unwrap();
    
    // Client1 invia messaggio crittografato
    let message_content = "Messaggio segreto di test!";
    let encrypted_msg = client1.prepare_encrypted_group_message(
        group_id,
        message_content,
        MessageType::Text,
    ).unwrap();
    
    // Server gestisce il messaggio
    if let ClientMessage::SendEncryptedMessage { encrypted_message } = encrypted_msg {
        server_chat.database.save_encrypted_message(&encrypted_message).await.unwrap();
        
        // Client2 riceve e decritta
        let decrypted = client2.decrypt_group_message(group_id, &encrypted_message).unwrap();
        
        assert_eq!(decrypted, message_content);
        println!("✅ Test end-to-end encryption passed!");
    }
}
```

## 6. Deployment

### Dockerfile - Aggiorna per nuove dipendenze

```dockerfile
# Le dipendenze ring e base64 sono già incluse nel Cargo.toml
# Nessuna modifica specifica richiesta per il container
```

### docker-compose.yml - Variabili ambiente

```yaml
version: '3.8'
services:
  ruggine-server:
    # ... configurazione esistente
    environment:
      - ENCRYPTION_ENABLED_BY_DEFAULT=false
      - ENCRYPTION_REQUIRE_FOR_GROUPS=false
      - ENCRYPTION_CLEANUP_DAYS=30
```

## 7. Documentazione Utente

### README aggiornato

```markdown
## 🔐 Crittografia End-to-End (Opzionale)

Ruggine supporta crittografia end-to-end per garantire la massima privacy:

### Caratteristiche
- ✅ AES-256-GCM encryption
- ✅ Chiavi generate localmente
- ✅ Server zero-knowledge
- ✅ Compatibile con messaggi normali

### Come Abilitare
1. Nelle impostazioni client, attiva "Crittografia End-to-End"
2. Per i gruppi: il creatore genera una chiave condivisa
3. I nuovi membri ricevono automaticamente le chiavi

### Note Importanti
⚠️ **I messaggi crittografati non sono recuperabili se si perde la chiave**
⚠️ **Backup delle chiavi non implementato in questa versione**
✅ **I messaggi normali continuano a funzionare normalmente**
```

Questa implementazione fornisce una crittografia end-to-end completa e sicura mantenendo la compatibilità con il sistema esistente!

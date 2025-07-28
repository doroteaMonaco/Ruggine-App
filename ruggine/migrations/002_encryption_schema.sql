-- Aggiunta delle tabelle per la crittografia end-to-end
-- Da eseguire dopo la migrazione iniziale

-- Tabella per i messaggi crittografati
CREATE TABLE IF NOT EXISTS encrypted_messages (
    id TEXT PRIMARY KEY,
    sender_id TEXT NOT NULL,
    group_id TEXT,
    receiver_id TEXT,
    encrypted_content TEXT NOT NULL,  -- Contenuto crittografato in Base64
    nonce TEXT NOT NULL,             -- Nonce per la decrittografia in Base64
    timestamp TEXT NOT NULL,
    message_type TEXT NOT NULL DEFAULT 'Text',
    created_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (sender_id) REFERENCES users(id),
    FOREIGN KEY (group_id) REFERENCES groups(id),
    FOREIGN KEY (receiver_id) REFERENCES users(id)
);

-- Tabella per le chiavi di crittografia dei gruppi
CREATE TABLE IF NOT EXISTS group_encryption_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id TEXT NOT NULL,
    encrypted_key TEXT NOT NULL,     -- Chiave crittografata per il gruppo
    created_by TEXT NOT NULL,        -- Chi ha creato/aggiornato la chiave
    created_at TEXT NOT NULL,
    expires_at TEXT,                 -- Scadenza opzionale per rotazione chiavi
    is_active BOOLEAN DEFAULT TRUE,
    FOREIGN KEY (group_id) REFERENCES groups(id),
    FOREIGN KEY (created_by) REFERENCES users(id)
);

-- Tabella per condividere chiavi tra utenti (per chat dirette)
CREATE TABLE IF NOT EXISTS user_encryption_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user1_id TEXT NOT NULL,
    user2_id TEXT NOT NULL,
    encrypted_key TEXT NOT NULL,     -- Chiave condivisa crittografata
    created_at TEXT NOT NULL,
    expires_at TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    FOREIGN KEY (user1_id) REFERENCES users(id),
    FOREIGN KEY (user2_id) REFERENCES users(id),
    UNIQUE(user1_id, user2_id)
);

-- Indici per migliorare le performance
CREATE INDEX IF NOT EXISTS idx_encrypted_messages_group_timestamp ON encrypted_messages(group_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_encrypted_messages_direct_timestamp ON encrypted_messages(sender_id, receiver_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_group_encryption_keys_group ON group_encryption_keys(group_id, created_at);
CREATE INDEX IF NOT EXISTS idx_user_encryption_keys_users ON user_encryption_keys(user1_id, user2_id);

-- Trigger per mantenere solo l'ultima chiave attiva per gruppo
CREATE TRIGGER IF NOT EXISTS update_group_key_status
AFTER INSERT ON group_encryption_keys
BEGIN
    UPDATE group_encryption_keys 
    SET is_active = FALSE 
    WHERE group_id = NEW.group_id 
      AND id != NEW.id 
      AND is_active = TRUE;
END;

-- Trigger per gestire la pulizia automatica dei messaggi vecchi (opzionale)
-- Questo trigger può essere abilitato se si vuole auto-eliminare i messaggi dopo un certo periodo
/*
CREATE TRIGGER IF NOT EXISTS cleanup_old_encrypted_messages
AFTER INSERT ON encrypted_messages
BEGIN
    DELETE FROM encrypted_messages 
    WHERE timestamp < datetime('now', '-30 days');
END;
*/

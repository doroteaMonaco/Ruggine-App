-- Migration 003: Friendship System
-- Aggiunge le tabelle per gestire richieste di amicizia e amicizie

-- Tabella per le richieste di amicizia
CREATE TABLE IF NOT EXISTS friend_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sender_id TEXT NOT NULL,           -- UUID dell'utente che invia la richiesta
    receiver_id TEXT NOT NULL,         -- UUID dell'utente che riceve la richiesta
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'accepted', 'rejected'
    message TEXT,                      -- Messaggio opzionale con la richiesta
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (receiver_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(sender_id, receiver_id),    -- Previene richieste duplicate
    CHECK (sender_id != receiver_id),  -- Non si può mandare richiesta a se stessi
    CHECK (status IN ('pending', 'accepted', 'rejected'))
);

-- Tabella per le amicizie confermate (per query più veloci)
CREATE TABLE IF NOT EXISTS friendships (
    user1_id TEXT NOT NULL,
    user2_id TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    PRIMARY KEY (user1_id, user2_id),
    FOREIGN KEY (user1_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (user2_id) REFERENCES users(id) ON DELETE CASCADE,
    CHECK (user1_id < user2_id)        -- Ordine canonico per evitare duplicati (user1 < user2)
);

-- Indici per performance
CREATE INDEX IF NOT EXISTS idx_friend_requests_sender ON friend_requests(sender_id);
CREATE INDEX IF NOT EXISTS idx_friend_requests_receiver ON friend_requests(receiver_id);
CREATE INDEX IF NOT EXISTS idx_friend_requests_status ON friend_requests(status);
CREATE INDEX IF NOT EXISTS idx_friendships_user1 ON friendships(user1_id);
CREATE INDEX IF NOT EXISTS idx_friendships_user2 ON friendships(user2_id);

-- Trigger per aggiornare updated_at automaticamente
CREATE TRIGGER IF NOT EXISTS update_friend_requests_timestamp 
AFTER UPDATE ON friend_requests
BEGIN
    UPDATE friend_requests SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- View per semplificare le query di amicizia (bidirezionale)
CREATE VIEW IF NOT EXISTS user_friendships AS
SELECT 
    user1_id as user_id, 
    user2_id as friend_id, 
    created_at
FROM friendships
UNION ALL
SELECT 
    user2_id as user_id, 
    user1_id as friend_id, 
    created_at
FROM friendships;

-- Inserimenti di test (opzionali, da rimuovere in produzione)
-- INSERT INTO friend_requests (sender_id, receiver_id, message) 
-- VALUES ('test-user-1', 'test-user-2', 'Ciao! Vuoi essere mio amico?');

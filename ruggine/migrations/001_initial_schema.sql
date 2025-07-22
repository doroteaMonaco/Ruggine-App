-- Tabella per gli utenti
CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT UNIQUE NOT NULL,
    created_at TEXT NOT NULL,
    last_seen TEXT,
    is_online BOOLEAN DEFAULT FALSE,
    -- Indici per performance
    UNIQUE(username)
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
    role TEXT DEFAULT 'member', -- 'admin', 'moderator', 'member'
    PRIMARY KEY (group_id, user_id),
    FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) 
);

-- Tabella per i messaggi
CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    sender_id TEXT NOT NULL,
    group_id TEXT, -- NULL per messaggi diretti
    receiver_id TEXT, -- Per messaggi diretti (quando group_id è NULL)
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
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'accepted', 'rejected', 'expired'
    responded_at TEXT,
    FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE,
    FOREIGN KEY (inviter_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (invitee_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(group_id, invitee_id, status) -- Previene inviti duplicati pendenti
);

-- Indici per ottimizzare le query più frequenti
CREATE INDEX idx_messages_group_timestamp ON messages(group_id, timestamp DESC);
CREATE INDEX idx_messages_sender_timestamp ON messages(sender_id, timestamp DESC);
CREATE INDEX idx_messages_receiver_timestamp ON messages(receiver_id, timestamp DESC);
CREATE INDEX idx_group_members_user ON group_members(user_id);
CREATE INDEX idx_group_members_group ON group_members(group_id);
CREATE INDEX idx_invites_invitee_status ON group_invites(invitee_id, status);
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_online ON users(is_online, last_seen DESC);

#![allow(dead_code)]

use anyhow::{Result, anyhow};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use uuid::Uuid;
use std::collections::HashMap;

/// Gestore della crittografia end-to-end per i messaggi
#[allow(dead_code)]
pub struct CryptoManager {
    rng: SystemRandom,
    /// Chiavi di sessione per gruppi (group_id -> chiave)
    group_keys: HashMap<Uuid, Vec<u8>>,
    /// Chiavi di sessione per chat dirette (user_pair_id -> chiave)
    direct_keys: HashMap<String, Vec<u8>>,
}

/// Messaggio crittografato pronto per l'invio
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedMessage {
    pub encrypted_content: String, // Base64 encoded
    pub nonce: String,            // Base64 encoded
    pub sender_id: Uuid,
    pub sender_username: String,  // Username del sender per evitare lookup aggiuntivi
    pub group_id: Option<Uuid>,
    pub receiver_id: Option<Uuid>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub message_type: crate::common::models::MessageType,
}

/// Chiave di crittografia per un gruppo o chat diretta
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CryptoKey {
    pub key_data: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl CryptoManager {
    /// Crea un nuovo gestore di crittografia
    pub fn new() -> Self {
        Self {
            rng: SystemRandom::new(),
            group_keys: HashMap::new(),
            direct_keys: HashMap::new(),
        }
    }

    /// Genera una nuova chiave di crittografia casuale
    pub fn generate_key(&self) -> Result<Vec<u8>> {
        let mut key_bytes = vec![0u8; 32]; // 256 bit per AES-256
        self.rng.fill(&mut key_bytes)
            .map_err(|_| anyhow!("Errore nella generazione della chiave"))?;
        Ok(key_bytes)
    }

    /// Genera un nonce casuale
    fn generate_nonce(&self) -> Result<[u8; NONCE_LEN]> {
        let mut nonce = [0u8; NONCE_LEN];
        self.rng.fill(&mut nonce)
            .map_err(|_| anyhow!("Errore nella generazione del nonce"))?;
        Ok(nonce)
    }

    /// Registra una chiave per un gruppo
    pub fn set_group_key(&mut self, group_id: Uuid, key: Vec<u8>) {
        self.group_keys.insert(group_id, key);
    }

    /// Registra una chiave per una chat diretta
    pub fn set_direct_key(&mut self, user1_id: Uuid, user2_id: Uuid, key: Vec<u8>) {
        let pair_id = self.get_user_pair_id(user1_id, user2_id);
        self.direct_keys.insert(pair_id, key);
    }

    /// Genera un ID univoco per una coppia di utenti (ordinato)
    fn get_user_pair_id(&self, user1_id: Uuid, user2_id: Uuid) -> String {
        let mut ids = vec![user1_id.to_string(), user2_id.to_string()];
        ids.sort();
        ids.join(":")
    }

    /// Critta un messaggio di gruppo
    pub fn encrypt_group_message(
        &self,
        group_id: Uuid,
        sender_id: Uuid,
        sender_username: &str,
        content: &str,
        message_type: crate::common::models::MessageType,
    ) -> Result<EncryptedMessage> {
        let key = self.group_keys.get(&group_id)
            .ok_or_else(|| anyhow!("Chiave del gruppo non trovata"))?;
        
        let (encrypted_content, nonce) = self.encrypt_content(content, key)?;
        
        Ok(EncryptedMessage {
            encrypted_content: BASE64.encode(&encrypted_content),
            nonce: BASE64.encode(&nonce),
            sender_id,
            sender_username: sender_username.to_string(),
            group_id: Some(group_id),
            receiver_id: None,
            timestamp: chrono::Utc::now(),
            message_type,
        })
    }

    /// Critta un messaggio diretto
    pub fn encrypt_direct_message(
        &self,
        sender_id: Uuid,
        sender_username: &str,
        receiver_id: Uuid,
        content: &str,
        message_type: crate::common::models::MessageType,
    ) -> Result<EncryptedMessage> {
        let pair_id = self.get_user_pair_id(sender_id, receiver_id);
        let key = self.direct_keys.get(&pair_id)
            .ok_or_else(|| anyhow!("Chiave della chat diretta non trovata"))?;
        
        let (encrypted_content, nonce) = self.encrypt_content(content, key)?;
        
        Ok(EncryptedMessage {
            encrypted_content: BASE64.encode(&encrypted_content),
            nonce: BASE64.encode(&nonce),
            sender_id,
            sender_username: sender_username.to_string(),
            group_id: None,
            receiver_id: Some(receiver_id),
            timestamp: chrono::Utc::now(),
            message_type,
        })
    }

    /// Decritta un messaggio di gruppo
    pub fn decrypt_group_message(
        &self,
        group_id: Uuid,
        encrypted_msg: &EncryptedMessage,
    ) -> Result<String> {
        let key = self.group_keys.get(&group_id)
            .ok_or_else(|| anyhow!("Chiave del gruppo non trovata"))?;
        
        let encrypted_content = BASE64.decode(&encrypted_msg.encrypted_content)
            .map_err(|e| anyhow!("Errore nel decodificare il contenuto: {}", e))?;
        let nonce_bytes = BASE64.decode(&encrypted_msg.nonce)
            .map_err(|e| anyhow!("Errore nel decodificare il nonce: {}", e))?;
        
        self.decrypt_content(&encrypted_content, &nonce_bytes, key)
    }

    /// Decritta un messaggio diretto
    pub fn decrypt_direct_message(
        &self,
        user1_id: Uuid,
        user2_id: Uuid,
        encrypted_msg: &EncryptedMessage,
    ) -> Result<String> {
        let pair_id = self.get_user_pair_id(user1_id, user2_id);
        let key = self.direct_keys.get(&pair_id)
            .ok_or_else(|| anyhow!("Chiave della chat diretta non trovata"))?;
        
        let encrypted_content = BASE64.decode(&encrypted_msg.encrypted_content)
            .map_err(|e| anyhow!("Errore nel decodificare il contenuto: {}", e))?;
        let nonce_bytes = BASE64.decode(&encrypted_msg.nonce)
            .map_err(|e| anyhow!("Errore nel decodificare il nonce: {}", e))?;
        
        self.decrypt_content(&encrypted_content, &nonce_bytes, key)
    }

    /// Funzione interna per crittografare il contenuto
    fn encrypt_content(&self, content: &str, key: &[u8]) -> Result<(Vec<u8>, [u8; NONCE_LEN])> {
        let unbound_key = UnboundKey::new(&AES_256_GCM, key)
            .map_err(|_| anyhow!("Errore nella creazione della chiave"))?;
        let sealing_key = LessSafeKey::new(unbound_key);
        
        let nonce = self.generate_nonce()?;
        let nonce_obj = Nonce::assume_unique_for_key(nonce);
        
        let mut in_out = content.as_bytes().to_vec();
        sealing_key.seal_in_place_append_tag(nonce_obj, Aad::empty(), &mut in_out)
            .map_err(|_| anyhow!("Errore nella crittografia"))?;
        
        Ok((in_out, nonce))
    }

    /// Funzione interna per decrittografare il contenuto
    fn decrypt_content(&self, encrypted_content: &[u8], nonce_bytes: &[u8], key: &[u8]) -> Result<String> {
        if nonce_bytes.len() != NONCE_LEN {
            return Err(anyhow!("Lunghezza del nonce non valida"));
        }
        
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(nonce_bytes);
        
        let unbound_key = UnboundKey::new(&AES_256_GCM, key)
            .map_err(|_| anyhow!("Errore nella creazione della chiave"))?;
        let opening_key = LessSafeKey::new(unbound_key);
        
        let nonce_obj = Nonce::assume_unique_for_key(nonce);
        
        let mut in_out = encrypted_content.to_vec();
        let decrypted = opening_key.open_in_place(nonce_obj, Aad::empty(), &mut in_out)
            .map_err(|_| anyhow!("Errore nella decrittografia"))?;
        
        String::from_utf8(decrypted.to_vec())
            .map_err(|e| anyhow!("Errore nella conversione UTF-8: {}", e))
    }

    /// Esporta una chiave per condividerla (per i nuovi membri del gruppo)
    pub fn export_group_key(&self, group_id: Uuid) -> Result<String> {
        let key = self.group_keys.get(&group_id)
            .ok_or_else(|| anyhow!("Chiave del gruppo non trovata"))?;
        Ok(BASE64.encode(key))
    }

    /// Importa una chiave condivisa
    pub fn import_group_key(&mut self, group_id: Uuid, encoded_key: &str) -> Result<()> {
        let key = BASE64.decode(encoded_key)
            .map_err(|e| anyhow!("Errore nel decodificare la chiave: {}", e))?;
        self.group_keys.insert(group_id, key);
        Ok(())
    }

        /// Controlla se una chiave di gruppo è disponibile
    pub fn has_group_key(&self, group_id: Uuid) -> bool {
        self.group_keys.contains_key(&group_id)
    }

    /// Controlla se una chiave per chat diretta è disponibile  
    pub fn has_direct_key(&self, user1_id: Uuid, user2_id: Uuid) -> bool {
        let pair_id = self.get_user_pair_id(user1_id, user2_id);
        self.direct_keys.contains_key(&pair_id)
    }

    /// Cleanup delle chiavi non più necessarie
    pub fn cleanup_keys(&mut self, active_groups: &[Uuid], active_direct_chats: &[(Uuid, Uuid)]) {
        // Rimuovi chiavi di gruppi non più attivi
        self.group_keys.retain(|group_id, _| active_groups.contains(group_id));
        
        // Rimuovi chiavi di chat dirette non più attive
        let active_pair_ids: Vec<String> = active_direct_chats.iter()
            .map(|(u1, u2)| self.get_user_pair_id(*u1, *u2))
            .collect();
        
        self.direct_keys.retain(|pair_id, _| active_pair_ids.contains(pair_id));
    }
}


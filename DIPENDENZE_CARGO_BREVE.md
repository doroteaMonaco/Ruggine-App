# Dipendenze Cargo - Analisi Breve

## 🗄️ **SQLx v0.7** - Database ORM Asincrono
**Features**: `["runtime-tokio-rustls", "sqlite", "chrono", "uuid", "macros"]`
- **Dove**: `src/server/database.rs`, `src/server/auth.rs`, `src/server/messages.rs`
- **Per cosa**: Query type-safe compile-time, gestione utenti/messaggi/sessioni, migrazioni DB
```rust
sqlx::query!("SELECT id, username FROM users WHERE is_online = 1")
```

## ⚠️ **Anyhow v1.0** - Error Handling
- **Dove**: Ovunque nei `Result<T, anyhow::Error>`
- **Per cosa**: Chain di errori user-friendly, propagazione errori con `?`

## 🔧 **Dotenvy v0.15** - Environment Variables
- **Dove**: `src/server/main.rs`, `src/server/config.rs`
- **Per cosa**: Caricamento `.env` per configurazioni (DB_URL, TLS_CERT_PATH, LOG_LEVEL)

## 🔐 **Argon2 v0.5** - Password Hashing
- **Dove**: `src/common/crypto.rs`, `src/server/auth.rs`
- **Per cosa**: Hash sicuri password utenti (OWASP compliant)
```rust
Argon2::default().hash_password(password.as_bytes(), &salt)
```

## 🎲 **Rand v0.8** - Random Generation
- **Dove**: `src/server/auth.rs`, `src/common/crypto.rs`
- **Per cosa**: Salt casuali, nonce encryption, session tokens random

## 🔒 **Ring v0.17** - Cryptography
- **Dove**: `src/common/crypto.rs`
- **Per cosa**: AES-256-GCM encryption messaggi, chiavi derivate, HMAC

## 📦 **Base64 v0.22** - Encoding
- **Dove**: `src/common/crypto.rs`, `src/server/messages.rs`
- **Per cosa**: Encoding ciphertext/nonce per storage DB JSON

## #️⃣ **MD5 v0.7** - Hashing (Legacy)
- **Dove**: `src/server/auth.rs`
- **Per cosa**: Hash aggiuntivo nei session token (non per password!)

## 🔑 **Keyring v1.1** - Credential Storage
- **Dove**: `src/client/utils/session_store.rs`
- **Per cosa**: Salvataggio sicuro session token nel sistema operativo

## 🛡️ **Rustls v0.21** - TLS Implementation
- **Dove**: `src/server/main.rs`, `src/server/connection.rs`
- **Per cosa**: TLS server puro Rust (no OpenSSL), certificati X.509

## 🔌 **Tokio-Rustls v0.24** - Async TLS
- **Dove**: `src/server/connection.rs`
- **Per cosa**: TLS asincrono su TCP streams, handshake non-bloccante

## 📄 **Rustls-Pemfile v1.0** - Certificate Parsing
- **Dove**: `src/server/connection.rs`
- **Per cosa**: Parsing certificati .pem e chiavi private per TLS

## 🔗 **Tokio-Tungstenite v0.21** - WebSocket
- **Dove**: `src/server/websocket.rs`, `src/client/services/websocket_client.rs`
- **Per cosa**: WebSocket real-time per chat, async message streaming

## 🔄 **Futures-Util v0.3** - Async Utilities
- **Dove**: `src/server/websocket.rs`
- **Per cosa**: `SinkExt`, `StreamExt` per WebSocket split streams

## 🌐 **URL v2.5** - URL Parsing
- **Dove**: `src/client/services/websocket_client.rs`
- **Per cosa**: Parsing e validazione URL WebSocket connection

## 🗃️ **Redis v0.24** - Cache Layer
**Features**: `["tokio-comp", "connection-manager"]`
- **Dove**: `src/server/redis_cache.rs`
- **Per cosa**: Cache messaggi temporanea, session storage distribuito, connection pooling

---

## 🔗 **Stack Integration**

```
TLS (Rustls) → WebSocket (Tungstenite) → JSON (Serde) → Database (SQLx) → Cache (Redis)
             ↓
Password (Argon2) → Encryption (Ring) → Storage (Base64) → Keyring (OS)
```

**Tutte queste dipendenze lavorano insieme per creare un sistema di chat sicuro, scalabile e real-time!**

---

## 📋 **Riepilogo per Categoria**

### 🔐 **Security Stack**
- **Argon2**: Password hashing
- **Ring**: Message encryption
- **Rustls**: TLS transport
- **Keyring**: Credential storage

### 🌐 **Network Stack**
- **Tokio-Tungstenite**: WebSocket real-time
- **Tokio-Rustls**: Async TLS
- **Futures-Util**: Stream processing

### 🗄️ **Data Stack**
- **SQLx**: Database ORM
- **Redis**: Caching layer
- **Base64**: Binary encoding
- **URL**: Connection parsing

### 🛠️ **Infrastructure**
- **Anyhow**: Error handling
- **Dotenvy**: Configuration
- **Rand**: Random generation
- **MD5**: Legacy hashing
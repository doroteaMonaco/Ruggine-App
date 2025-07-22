# Database Architecture - Ruggine Chat Application

## Panoramica

L'applicazione Ruggine utilizza SQLite come database embedded per garantire la massima compatibilità cross-platform e semplicità di deployment, rispettando tutti i requisiti della traccia del progetto.

## Indice della Documentazione

1. [**Schema del Database**](schema.md) - Struttura tabelle e relazioni
2. [**Indici e Performance**](indexes.md) - Ottimizzazioni e strategia di indicizzazione
3. [**API Database**](api.md) - Funzioni e operazioni disponibili
4. [**Migrazioni**](migrations.md) - Sistema di versioning dello schema
5. [**Performance Monitoring**](monitoring.md) - Sistema di logging CPU (requisito traccia)
6. [**Deployment**](deployment.md) - Setup e configurazione cross-platform

## Requisiti Traccia Soddisfatti

### ✅ **Cross-Platform (Almeno 2 Piattaforme)**
- **Windows**: SQLite nativo, zero dipendenze
- **Linux**: SQLite nativo, compilazione con cargo
- **MacOS**: SQLite nativo, supporto nativo
- **Android/iOS**: SQLite embedded supportato da Rust

### ✅ **Chat con Gruppi e Inviti**
- Tabella `groups` per gestione gruppi
- Tabella `group_members` per relazioni many-to-many
- Tabella `group_invites` con sistema di stati (pending/accepted/rejected)
- Supporto messaggi diretti e di gruppo

### ✅ **Registrazione al Primo Avvio**
- Tabella `users` con sistema di registrazione
- Controllo unicità username
- Tracking primo accesso

### ✅ **Logging CPU Ogni 2 Minuti**
- Tabella `performance_metrics` dedicata
- Timestamp, CPU usage, memoria, connessioni attive
- Cleanup automatico dati vecchi

### ✅ **Ottimizzazione Performance e Dimensioni**
- Database embedded (no server esterno)
- Indici ottimizzati per query frequenti
- Connection pooling con SQLx
- WAL mode per prestazioni
- Dimensioni minime dell'eseguibile

## Caratteristiche Tecniche

### **Tecnologia Scelta: SQLite**

**Vantaggi:**
- **Zero Configuration**: Nessun setup database server
- **Embedded**: Database incluso nell'applicativo
- **Cross-Platform**: Funziona identicamente su tutte le piattaforme
- **ACID Compliance**: Garanzie di integrità dei dati
- **Performance**: Ottimizzato per applicazioni locali
- **Dimensioni**: Minimo overhead per l'eseguibile

**Specifiche:**
- **Engine**: SQLite 3.x via sqlx-rs
- **Journal Mode**: WAL (Write-Ahead Logging) per performance
- **Connection Pool**: Gestito da SQLx
- **Backup**: File-based, facile da spostare/copiare

### **Architettura Dati**

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│     users       │    │     groups      │    │   messages      │
│                 │    │                 │    │                 │
│ - id (UUID)     │    │ - id (UUID)     │    │ - id (UUID)     │
│ - username      │    │ - name          │    │ - sender_id     │
│ - created_at    │    │ - created_by    │    │ - group_id      │
│ - is_online     │    │ - members       │    │ - receiver_id   │
└─────────────────┘    └─────────────────┘    │ - content       │
         │                       │             │ - timestamp     │
         └───────────────┬───────┘             └─────────────────┘
                        │
               ┌─────────────────┐
               │ group_members   │
               │                 │
               │ - group_id      │
               │ - user_id       │
               │ - role          │
               │ - joined_at     │
               └─────────────────┘
```

### **Sistema di Inviti**

```
┌─────────────────┐
│ group_invites   │
│                 │
│ - id (UUID)     │
│ - group_id      │
│ - inviter_id    │
│ - invitee_id    │
│ - status        │
│ - created_at    │
│ - expires_at    │
└─────────────────┘
```

### **Performance Monitoring (Requisito Traccia)**

```
┌──────────────────────┐
│ performance_metrics  │
│                      │
│ - timestamp          │
│ - cpu_usage_percent  │ ← Logging ogni 2 minuti
│ - memory_usage_mb    │
│ - active_connections │
│ - messages_per_min   │
└──────────────────────┘
```

## Flusso Operativo

### **1. Primo Avvio**
```
1. Server avvia → SQLx connect "sqlite:ruggine.db"
2. Se DB non esiste → Crea file vuoto
3. Applica migrazioni → Schema completo
4. Server pronto per connessioni
```

### **2. Registrazione Utente (Primo Avvio Programma)**
```
1. Client invia /register <username>
2. Controllo unicità username
3. Creazione UUID utente
4. Inserimento in tabella users
5. Conferma registrazione
```

### **3. Gestione Gruppi**
```
1. Creazione gruppo → Inserimento in groups + group_members
2. Invito utente → Inserimento in group_invites (status: pending)
3. Accettazione → Update status + Inserimento in group_members
4. Messaggio gruppo → Controllo membership + Inserimento message
```

### **4. Performance Monitoring**
```
Ogni 2 minuti:
1. Raccolta metriche CPU (sysinfo)
2. Conteggio connessioni attive
3. Calcolo messaggi/minuto
4. Inserimento in performance_metrics
5. Log su file ruggine_performance.log
```

## File Generati

Al primo avvio del server vengono creati:

```
ruggine/
├── ruggine.db                    ← Database SQLite principale
├── ruggine.db-wal               ← Write-Ahead Log (performance)
├── ruggine.db-shm               ← Shared Memory (performance)
├── ruggine_performance.log      ← Log CPU ogni 2 minuti (requisito)
└── ruggine_data.json           ← Backup JSON (opzionale)
```

## Sicurezza

- **SQL Injection**: Prevenuto con prepared statements SQLx
- **UUID**: Uso di UUID v4 per tutti gli ID (no enumeration)
- **Soft Delete**: Messaggi marcati come cancellati, non eliminati
- **Audit Trail**: Log completo di tutte le operazioni (tabella audit_log)
- **Input Validation**: Validazione username, nomi gruppi, contenuti

## Scalabilità

### **Limiti Attuali (SQLite)**
- **Utenti Simultanei**: ~1000 connessioni (più che sufficienti per la traccia)
- **Dimensione DB**: Fino a 281 TB (teorico, praticamente illimitato)
- **Performance**: Ottimale fino a 100K+ messaggi

### **Migrazione Futura (Se Necessario)**
- Schema compatibile con PostgreSQL/MySQL
- Migrazioni SQLx funzionano su multiple piattaforme
- API DatabaseManager astratta dal DBMS specifico

## Conformità Traccia

| Requisito | Implementazione | Stato |
|-----------|----------------|-------|
| Chat gruppi + inviti | Tabelle groups, group_members, group_invites | ✅ |
| Registrazione primo avvio | Tabella users, controllo unicità | ✅ |
| Cross-platform (≥2) | SQLite: Windows, Linux, MacOS, Android, iOS | ✅ |
| Performance CPU/dimensioni | Indici, connection pool, embedded DB | ✅ |
| Log CPU ogni 2 minuti | Tabella performance_metrics + file log | ✅ |
| Dimensioni eseguibile | Database embedded, zero dipendenze runtime | ✅ |

---

**Documentazione Completa**: Vedi i file specifici per dettagli implementativi e guide operative.

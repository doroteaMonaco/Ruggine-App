# 🎯 Ruggine Chat - Funzionalità Implementate

## ✅ **Comandi Chat Completamente Implementati**

### **Autenticazione & Utenti**
- **`/register <username>`** - Registrazione con username univoco
- **`/users`** - Lista utenti online
- **`/help`** - Mostra tutti i comandi disponibili
- **`/quit`** - Disconnessione dal server

### **Gestione Gruppi**
- **`/create_group <name>`** - Crea un nuovo gruppo
- **`/my_groups`** - Lista dei tuoi gruppi
- **`/leave_group <group_name>`** - Lascia un gruppo

### **Sistema Inviti**
- **`/invite <username> <group_name>`** - Invita un utente a un gruppo
- **`/accept_invite <invite_id>`** - Accetta un invito al gruppo
- **`/reject_invite <invite_id>`** - Rifiuta un invito al gruppo  
- **`/my_invites`** - Lista inviti pendenti

### **Messaggistica**
- **`/send <group_name> <message>`** - Invia messaggio a un gruppo
- **`/send_private <username> <message>`** - Invia messaggio privato

---

## 🚀 **Modalità di Connessione Automatizzate**

### **Per l'Host del Server (tu):**
```bash
cargo run --bin ruggine-client -- --auto --username "Luigi"
```
- Usa automaticamente `127.0.0.1:5000` (localhost)

### **Per Client Remoti (amici):**
```bash
cargo run --bin ruggine-client -- --remote --username "Mario"
```
- Usa automaticamente `95.234.28.229:5000` (il tuo IP pubblico)

### **Modalità Manuale:**
```bash
cargo run --bin ruggine-client -- --username "Giuseppe" --host "95.234.28.229"
```

---

## 🔧 **Configurazione (file .env)**

```properties
# Server Configuration
SERVER_HOST=0.0.0.0          # Accetta connessioni da qualsiasi IP
SERVER_PORT=5000

# Client Configuration  
CLIENT_DEFAULT_HOST=127.0.0.1    # Per connessioni locali (host)
CLIENT_PUBLIC_HOST=95.234.28.229 # Per connessioni remote (guest)
CLIENT_DEFAULT_PORT=5000
```

---

## 📋 **Workflow Tipico di Utilizzo**

### **1. Avvio Server**
```bash
cargo run --bin ruggine-server
```

### **2. Host si connette**
```bash
cargo run --bin ruggine-client -- --auto --username "Luigi"
```

### **3. Amico remoto si connette**
```bash
cargo run --bin ruggine-client -- --remote --username "Mario"
```

### **4. Creazione gruppo e inviti**
```
Luigi> /create_group friends
Luigi> /invite Mario friends

Mario> /my_invites
Mario> /accept_invite 123e4567-e89b-12d3-a456-426614174000
```

### **5. Chat di gruppo**
```
Luigi> /send friends Ciao a tutti!
Mario> /send friends Ciao Luigi! Come va?
```

### **6. Messaggi privati**
```
Luigi> /send_private Mario Hai visto il match ieri?
```

---

## 🗄️ **Database & Persistenza**

- **SQLite database** (`data/ruggine.db`) per persistenza dati
- **Salvataggio automatico** di utenti, gruppi, messaggi e inviti
- **Configurazione centralizzata** tramite file `.env`
- **Username univoci** garantiti dal database

---

## 🌐 **Multi-Host Support**

✅ **Configurato per comunicazione Internet**
- Server listens su `0.0.0.0:5000` (tutte le interfacce)
- IP pubblico configurato: `95.234.28.229`
- Port forwarding necessario per accesso da Internet
- Connessioni locali e remote automatizzate via config

---

## ❌ **Funzionalità Rimosse (come richiesto)**

- **`/join_group`** - Sostituito da `/accept_invite`
- **`/save`** - Rimosso (salvataggio automatico)

---

## 🎯 **Ready for Multi-Host Communication!**

Il sistema è completamente funzionale per chat multi-host con:
- ✅ Registrazione semplice senza login complesso
- ✅ Username univoci garantiti
- ✅ Gestione gruppi con inviti
- ✅ Messaggi di gruppo e privati
- ✅ Connessione automatizzata via config
- ✅ Supporto multi-dispositivo/host

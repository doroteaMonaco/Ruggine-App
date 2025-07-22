# Performance Monitoring - Ruggine Database

## Requisito Traccia: Logging CPU ogni 2 minuti

Il sistema di performance monitoring implementa il requisito specifico della traccia di "generare un file di log, riportando i dettagli sull'utilizzo di CPU ogni 2 minuti".

## Architettura Monitoring

### **Componenti Sistema**

```rust
// Struttura per metriche performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage_percent: f64,      // Requisito traccia
    pub memory_usage_mb: f64,
    pub active_connections: usize,
    pub messages_per_minute: u64,
}

// Task monitoring nel server main
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(120)); // 2 minuti
    
    loop {
        interval.tick().await;
        
        // Raccolta metriche CPU (sysinfo)
        let mut system = sysinfo::System::new_all();
        system.refresh_cpu();
        let cpu_usage = system.cpus().iter()
            .map(|cpu| cpu.cpu_usage())
            .sum::<f32>() / system.cpus().len() as f32;
        
        // Salvataggio database + file log
        save_performance_metrics(cpu_usage).await;
    }
});
```

## Implementazione Database

### **Tabella Performance Metrics**

```sql
CREATE TABLE performance_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,                -- ISO 8601 timestamp
    cpu_usage_percent REAL NOT NULL,        -- Utilizzo CPU % (requisito traccia)
    memory_usage_mb REAL NOT NULL,          -- Utilizzo memoria MB
    active_connections INTEGER NOT NULL,     -- Connessioni client attive
    messages_per_minute INTEGER NOT NULL,    -- Throughput messaggi
    server_uptime_seconds INTEGER NOT NULL   -- Uptime server
);

-- Indice per query cronologiche
CREATE INDEX idx_performance_timestamp ON performance_metrics(timestamp DESC);
```

### **API Salvataggio Metriche**

```rust
impl DatabaseManager {
    /// Salva metriche performance ogni 2 minuti (requisito traccia)
    pub async fn save_performance_metrics(&self, metrics: &PerformanceMetrics) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO performance_metrics 
            (timestamp, cpu_usage_percent, memory_usage_mb, active_connections, 
             messages_per_minute, server_uptime_seconds)
            VALUES (?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(metrics.timestamp.to_rfc3339())
        .bind(metrics.cpu_usage_percent)
        .bind(metrics.memory_usage_mb)
        .bind(metrics.active_connections as i64)
        .bind(metrics.messages_per_minute as i64)
        .bind(0i64) // server_uptime - da implementare
        .execute(&self.pool)
        .await?;

        info!("Performance metrics saved: CPU {:.1}%, Memory {:.1}MB, Connections {}", 
              metrics.cpu_usage_percent, metrics.memory_usage_mb, metrics.active_connections);
        
        Ok(())
    }
}
```

## File di Log Performance

### **Formato Log File**

Il sistema genera `ruggine_performance.log` con formato CSV per analisi:

```csv
# Ruggine Server Performance Log
# Timestamp, Active_Users, Groups, Total_Messages, CPU_Usage
2024-01-15 10:00:00 UTC, 15, 5, 1234, 12.5%
2024-01-15 10:02:00 UTC, 18, 5, 1267, 15.2%
2024-01-15 10:04:00 UTC, 22, 6, 1301, 18.7%
2024-01-15 10:06:00 UTC, 19, 6, 1334, 14.1%
```

### **Implementazione Logging**

```rust
// Nel task di monitoring del server main.rs
let log_file_path = "ruggine_performance.log";

// Crea header se file non esiste
if !std::path::Path::new(log_file_path).exists() {
    std::fs::write(log_file_path, 
        "# Ruggine Server Performance Log\n# Timestamp, Active_Users, Groups, Total_Messages, CPU_Usage\n"
    )?;
}

loop {
    interval.tick().await;
    
    // Raccolta metriche
    let (users, groups, messages) = chat_manager.get_performance_metrics().await;
    let cpu_usage = get_cpu_usage(); // sysinfo
    
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    
    // Log su console
    info!("📊 Performance Metrics - Active Users: {}, Groups: {}, Total Messages: {}", 
        users, groups, messages);
    info!("🖥️ CPU Usage: {:.1}%", cpu_usage);
    
    // Log su file (REQUISITO TRACCIA)
    let log_entry = format!("{}, {}, {}, {}, {:.1}%\n", 
        timestamp, users, groups, messages, cpu_usage);
    
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)?
        .write_all(log_entry.as_bytes())?;
    
    // Salva anche in database per analisi
    let metrics = PerformanceMetrics {
        timestamp: Utc::now(),
        cpu_usage_percent: cpu_usage as f64,
        memory_usage_mb: get_memory_usage(),
        active_connections: users,
        messages_per_minute: calculate_msg_per_minute().await,
    };
    
    db_manager.save_performance_metrics(&metrics).await?;
}
```

## Raccolta Metriche

### **CPU Usage (Requisito Traccia)**

```rust
use sysinfo::{System, SystemExt, CpuExt};

fn get_cpu_usage() -> f32 {
    let mut system = System::new_all();
    system.refresh_cpu();
    
    // Media utilizzo di tutti i core
    let total_usage: f32 = system.cpus().iter()
        .map(|cpu| cpu.cpu_usage())
        .sum();
    
    total_usage / system.cpus().len() as f32
}
```

### **Memory Usage**

```rust
fn get_memory_usage() -> f64 {
    let mut system = System::new_all();
    system.refresh_memory();
    
    let total_memory = system.total_memory();
    let used_memory = system.used_memory();
    
    (used_memory as f64) / (1024.0 * 1024.0) // MB
}
```

### **Active Connections**

```rust
// Nel ChatManager
pub async fn get_active_connections(&self) -> usize {
    self.users.read().await.len()
}
```

### **Messages Per Minute**

```rust
// Calcolo throughput messaggi
async fn calculate_messages_per_minute(&self) -> u64 {
    let one_minute_ago = Utc::now() - chrono::Duration::minutes(1);
    
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM messages WHERE timestamp > ?"
    )
    .bind(one_minute_ago.to_rfc3339())
    .fetch_one(&self.pool)
    .await
    .unwrap_or(0) as u64
}
```

## Query e Analisi Performance

### **Query Metriche Recenti**

```sql
-- Ultime 10 rilevazioni
SELECT 
    timestamp,
    cpu_usage_percent,
    memory_usage_mb,
    active_connections,
    messages_per_minute
FROM performance_metrics 
ORDER BY timestamp DESC 
LIMIT 10;
```

### **Analisi Trend CPU**

```sql
-- Media CPU ultima ora
SELECT 
    AVG(cpu_usage_percent) as avg_cpu,
    MAX(cpu_usage_percent) as max_cpu,
    MIN(cpu_usage_percent) as min_cpu
FROM performance_metrics 
WHERE timestamp > datetime('now', '-1 hour');
```

### **Identificazione Picchi**

```sql
-- Picchi CPU sopra soglia
SELECT 
    timestamp,
    cpu_usage_percent,
    active_connections,
    messages_per_minute
FROM performance_metrics 
WHERE cpu_usage_percent > 80.0
ORDER BY timestamp DESC;
```

### **Correlazione Performance**

```sql
-- Correlazione CPU vs Carico
SELECT 
    cpu_usage_percent,
    active_connections,
    messages_per_minute,
    (active_connections * messages_per_minute) as load_score
FROM performance_metrics 
WHERE timestamp > datetime('now', '-6 hours')
ORDER BY cpu_usage_percent DESC;
```

## Dashboard Performance

### **Metriche Real-Time**

```rust
// API per dashboard web/CLI
impl DatabaseManager {
    pub async fn get_performance_dashboard(&self) -> Result<PerformanceDashboard> {
        // Ultime 24 ore
        let last_24h = sqlx::query_as::<_, PerformanceMetrics>(
            "SELECT * FROM performance_metrics WHERE timestamp > datetime('now', '-24 hours') ORDER BY timestamp"
        ).fetch_all(&self.pool).await?;
        
        // Calcola statistiche
        let avg_cpu = last_24h.iter().map(|m| m.cpu_usage_percent).sum::<f64>() / last_24h.len() as f64;
        let max_cpu = last_24h.iter().map(|m| m.cpu_usage_percent).fold(0.0, f64::max);
        let current_load = last_24h.last().map(|m| m.active_connections).unwrap_or(0);
        
        Ok(PerformanceDashboard {
            avg_cpu_24h: avg_cpu,
            max_cpu_24h: max_cpu,
            current_connections: current_load,
            total_samples: last_24h.len(),
            data_points: last_24h,
        })
    }
}
```

### **Alerting System**

```rust
// Sistema di alert per performance critiche
async fn check_performance_alerts(metrics: &PerformanceMetrics) -> Result<()> {
    // Alert CPU alto (requisito traccia: monitoring)
    if metrics.cpu_usage_percent > 85.0 {
        warn!("🚨 HIGH CPU USAGE: {:.1}% - Consider scaling", metrics.cpu_usage_percent);
        
        // Log alert in audit
        db_manager.log_action(
            None,
            "ALERT",
            "performance",
            None,
            Some(&format!("{{\"type\": \"high_cpu\", \"value\": {}}}", metrics.cpu_usage_percent)),
            None
        ).await?;
    }
    
    // Alert memoria alta
    if metrics.memory_usage_mb > 1024.0 {
        warn!("🚨 HIGH MEMORY USAGE: {:.1}MB", metrics.memory_usage_mb);
    }
    
    // Alert connessioni
    if metrics.active_connections > 500 {
        warn!("🚨 HIGH CONNECTION COUNT: {}", metrics.active_connections);
    }
    
    Ok(())
}
```

## Cleanup e Manutenzione

### **Cleanup Automatico**

```rust
impl DatabaseManager {
    pub async fn cleanup_performance_data(&self, days_to_keep: i64) -> Result<()> {
        let cutoff_date = (Utc::now() - chrono::Duration::days(days_to_keep)).to_rfc3339();
        
        let deleted_count = sqlx::query(
            "DELETE FROM performance_metrics WHERE timestamp < ?"
        )
        .bind(&cutoff_date)
        .execute(&self.pool)
        .await?
        .rows_affected();
        
        info!("Performance cleanup: {} old metrics removed (keeping {} days)", 
              deleted_count, days_to_keep);
        
        Ok(())
    }
}
```

### **Rotazione Log File**

```rust
// Rotazione settimanale del file di log
async fn rotate_performance_log() -> Result<()> {
    let log_file = "ruggine_performance.log";
    
    if std::fs::metadata(log_file)?.len() > 10_000_000 { // 10MB
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let archived_name = format!("ruggine_performance_{}.log", timestamp);
        
        std::fs::rename(log_file, &archived_name)?;
        
        // Ricrea file con header
        std::fs::write(log_file, 
            "# Ruggine Server Performance Log\n# Timestamp, Active_Users, Groups, Total_Messages, CPU_Usage\n"
        )?;
        
        info!("Performance log rotated to: {}", archived_name);
    }
    
    Ok(())
}
```

## Conformità Requisiti Traccia

### **✅ Logging CPU ogni 2 minuti**

- **Frequenza**: Timer esatto 120 secondi (`Duration::from_secs(120)`)
- **Dettagli CPU**: Utilizzo percentuale medio di tutti i core
- **File di log**: `ruggine_performance.log` formato CSV
- **Persistenza**: Dati anche in database per analisi

### **✅ Attenzione alle Performance**

- **Monitoring continuo**: CPU, memoria, connessioni, throughput
- **Identificazione colli di bottiglia**: Query correlazione carico/performance
- **Alerting automatico**: Soglie configurabili per CPU/memoria
- **Cleanup automatico**: Rimozione dati vecchi per mantenere performance

### **✅ Ottimizzazione Dimensioni**

- **Indici mirati**: Solo su timestamp per query cronologiche
- **Cleanup regolare**: Ritenzione configurabile (default 7 giorni)
- **Formato efficiente**: Dati numerici compatti
- **Rotazione automatica**: File log limitati in dimensione

## Utilizzo Operativo

### **Avvio Server**

```bash
# Il monitoring parte automaticamente
cargo run --bin ruggine-server

# Output atteso ogni 2 minuti:
# INFO ruggine_server: 📊 Performance Metrics - Active Users: 15, Groups: 5, Total Messages: 1234
# INFO ruggine_server: 🖥️ CPU Usage: 12.5%
# INFO ruggine_server: 💾 Performance data logged to ruggine_performance.log
```

### **Analisi File Log**

```bash
# Visualizza ultimi records
tail -n 10 ruggine_performance.log

# Analisi trend CPU
awk -F', ' '{print $5}' ruggine_performance.log | tail -n 30

# Statistiche connessioni
awk -F', ' '{sum+=$2} END {print "Media utenti:", sum/NR}' ruggine_performance.log
```

### **Query Database**

```sql
-- Performance ultima ora
SELECT * FROM performance_metrics 
WHERE timestamp > datetime('now', '-1 hour') 
ORDER BY timestamp DESC;

-- Report giornaliero
SELECT 
    DATE(timestamp) as date,
    AVG(cpu_usage_percent) as avg_cpu,
    MAX(active_connections) as peak_users,
    SUM(messages_per_minute) as total_throughput
FROM performance_metrics 
GROUP BY DATE(timestamp)
ORDER BY date DESC;
```

---

**Next**: [Deployment](deployment.md) | [Schema](schema.md)

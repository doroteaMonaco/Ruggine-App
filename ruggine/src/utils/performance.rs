#![allow(dead_code)]

use crate::common::models::PerformanceMetrics;
use chrono::Utc;
use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;
use sysinfo::System;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Monitor delle prestazioni del sistema
#[allow(dead_code)]
pub struct PerformanceMonitor {
    system: Arc<RwLock<System>>,
    active_connections: Arc<RwLock<usize>>,
    messages_count: Arc<RwLock<u64>>,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            system: Arc::new(RwLock::new(System::new_all())),
            active_connections: Arc::new(RwLock::new(0)),
            messages_count: Arc::new(RwLock::new(0)),
        }
    }

    /// Avvia il monitoraggio automatico ogni 2 minuti
    pub async fn start_monitoring(&self) {
        let system = Arc::clone(&self.system);
        let active_connections = Arc::clone(&self.active_connections);
        let messages_count = Arc::clone(&self.messages_count);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(120)); // 2 minuti

            loop {
                interval.tick().await;
                
                let metrics = Self::collect_metrics(
                    &system,
                    &active_connections,
                    &messages_count,
                ).await;

                Self::log_metrics(&metrics).await;
            }
        });
    }

    /// Raccoglie le metriche di performance correnti
    async fn collect_metrics(
        system: &Arc<RwLock<System>>,
        active_connections: &Arc<RwLock<usize>>,
        messages_count: &Arc<RwLock<u64>>,
    ) -> PerformanceMetrics {
        let mut sys = system.write().await;
        sys.refresh_cpu();
        sys.refresh_memory();

        let cpu_usage = sys.cpus().iter()
            .map(|cpu| cpu.cpu_usage() as f64)
            .sum::<f64>() / sys.cpus().len() as f64;

        let memory_usage = sys.used_memory() as f64 / 1024.0 / 1024.0; // MB

        let connections = *active_connections.read().await;
        let messages = *messages_count.read().await;

        PerformanceMetrics {
            timestamp: Utc::now(),
            cpu_usage_percent: cpu_usage,
            memory_usage_mb: memory_usage,
            active_connections: connections,
            messages_per_minute: messages,
        }
    }

    /// Registra le metriche nel log
    async fn log_metrics(metrics: &PerformanceMetrics) {
        info!(
            "Performance Metrics [{}] - CPU: {:.2}%, Memory: {:.2}MB, Connections: {}, Messages/min: {}",
            metrics.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            metrics.cpu_usage_percent,
            metrics.memory_usage_mb,
            metrics.active_connections,
            metrics.messages_per_minute
        );

        // Avvisi se le performance sono critiche
        if metrics.cpu_usage_percent > 80.0 {
            warn!("High CPU usage detected: {:.2}%", metrics.cpu_usage_percent);
        }

        if metrics.memory_usage_mb > 1024.0 {
            warn!("High memory usage detected: {:.2}MB", metrics.memory_usage_mb);
        }
    }

    /// Aggiorna il contatore delle connessioni attive
    pub async fn set_active_connections(&self, count: usize) {
        *self.active_connections.write().await = count;
    }

    /// Incrementa il contatore dei messaggi
    pub async fn increment_message_count(&self) {
        *self.messages_count.write().await += 1;
    }

    /// Resetta il contatore dei messaggi (chiamato ogni 2 minuti)
    pub async fn reset_message_count(&self) {
        *self.messages_count.write().await = 0;
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

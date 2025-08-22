use crate::server::database::Database;
use std::sync::Arc;
use sqlx::Row;

pub async fn list_online(db: Arc<Database>) -> String {
    println!("[USERS] Listing online users");
    let rows = sqlx::query("SELECT username FROM users WHERE is_online = 1")
        .fetch_all(&db.pool)
        .await;
    match rows {
        Ok(rows) => {
            let users: Vec<String> = rows.iter().map(|r| r.get::<String,_>("username")).collect();
            format!("OK: Online users: {}", users.join(", "))
        }
        Err(e) => {
            println!("[USERS] Error listing online users: {}", e);
            format!("ERR: {}", e)
        }
    }
}

pub async fn list_all(db: Arc<Database>, exclude_username: Option<&str>) -> String {
    println!("[USERS] Listing all users");
    let rows = sqlx::query("SELECT username FROM users")
        .fetch_all(&db.pool)
        .await;
    match rows {
        Ok(rows) => {
            let mut users: Vec<String> = rows.iter().map(|r| r.get::<String,_>("username")).collect();
            if let Some(exclude) = exclude_username {
                users.retain(|u| u != exclude);
            }
            format!("OK: All users: {}", users.join(", "))
        }
        Err(e) => {
            println!("[USERS] Error listing all users: {}", e);
            format!("ERR: {}", e)
        }
    }
}

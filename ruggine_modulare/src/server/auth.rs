use crate::server::database::Database;
use std::sync::Arc;
use sqlx::Row;
use argon2::{self, Config};
use rand::{Rng, RngCore};

fn hash_password(password: &str) -> String {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    argon2::hash_encoded(password.as_bytes(), &salt, &Config::default()).unwrap()
}

fn verify_password(hash: &str, password: &str) -> bool {
    argon2::verify_encoded(hash, password.as_bytes()).unwrap_or(false)
}

fn generate_session_token() -> String {
    let uuid = uuid::Uuid::new_v4().to_string();
    let mut random = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut random);
    format!("{}-{:x}", uuid, md5::compute(&random))
}

pub async fn register(db: Arc<Database>, username: &str, password: &str) -> String {
    println!("[AUTH] Register attempt: {}", username);
    let user_id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp();
    let password_hash = hash_password(password);
    let tx = db.pool.begin().await;
    match tx {
        Ok(mut tx) => {
            let res = sqlx::query("INSERT INTO users (id, username, created_at, is_online) VALUES (?, ?, ?, 0)")
                .bind(&user_id)
                .bind(username)
                .bind(created_at)
                .execute(&mut *tx)
                .await;
            if let Err(e) = res {
                println!("[AUTH] Registration failed for {}: {}", username, e);
                return format!("ERR: Registration failed: {}", e);
            }
            sqlx::query("INSERT INTO user_encryption_keys (user_id, public_key, private_key) VALUES (?, '', '')")
                .bind(&user_id)
                .execute(&mut *tx)
                .await
                .ok();
            sqlx::query("INSERT INTO auth (user_id, password_hash) VALUES (?, ?)")
                .bind(&user_id)
                .bind(&password_hash)
                .execute(&mut *tx)
                .await
                .ok();
            tx.commit().await.ok();
            println!("[AUTH] Registered user {} (id={})", username, user_id);
            format!("OK: Registered as {}", username)
        }
        Err(e) => {
            println!("[AUTH] Registration failed for {}: {}", username, e);
            format!("ERR: Registration failed: {}", e)
        }
    }
}

pub async fn login(db: Arc<Database>, username: &str, password: &str) -> String {
    println!("[AUTH] Login attempt: {}", username);
    let row = sqlx::query("SELECT users.id, password_hash FROM users JOIN auth ON users.id = auth.user_id WHERE username = ?")
        .bind(username)
        .fetch_optional(&db.pool)
        .await;
    match row {
        Ok(Some(row)) => {
            let user_id: String = row.get("id");
            let password_hash: String = row.get("password_hash");
            if verify_password(&password_hash, password) {
                sqlx::query("UPDATE users SET is_online = 1 WHERE id = ?")
                    .bind(&user_id)
                    .execute(&db.pool)
                    .await
                    .ok();
                // Sessione
                let session_token = generate_session_token();
                let now = chrono::Utc::now().timestamp();
                let expires = now + 60*60*24*7; // 1 settimana
                sqlx::query("INSERT INTO sessions (user_id, session_token, created_at, expires_at) VALUES (?, ?, ?, ?)")
                    .bind(&user_id)
                    .bind(&session_token)
                    .bind(now)
                    .bind(expires)
                    .execute(&db.pool)
                    .await
                    .ok();
                println!("[AUTH] Login success for {} (id={})", username, user_id);
                format!("OK: Logged in as {}\nSESSION: {}", username, session_token)
            } else {
                println!("[AUTH] Login failed for {}: wrong password", username);
                "ERR: Wrong password".to_string()
            }
        }
        Ok(None) => {
            println!("[AUTH] Login failed for {}: user not found", username);
            "ERR: User not found".to_string()
        }
        Err(e) => {
            println!("[AUTH] Login failed for {}: {}", username, e);
            format!("ERR: Login failed: {}", e)
        }
    }
}

pub async fn validate_session(db: Arc<Database>, session_token: &str) -> Option<String> {
    let now = chrono::Utc::now().timestamp();
    let row = sqlx::query("SELECT user_id FROM sessions WHERE session_token = ? AND expires_at > ?")
        .bind(session_token)
        .bind(now)
        .fetch_optional(&db.pool)
        .await
        .ok()?;
    row.map(|r| r.get::<String,_>("user_id"))
}

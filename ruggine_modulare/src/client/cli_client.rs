use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, stdin};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:5000".to_string());
    println!("[CLIENT] Benvenuto! Digita i comandi (es: /register user pass, /login user pass):");
    let stream = TcpStream::connect(&addr).await?;
    let (reader, writer) = stream.into_split();
    let mut server_reader = BufReader::new(reader);
    let mut server_writer = BufWriter::new(writer);
    let mut input = BufReader::new(stdin());
    let mut input_line = String::new();
    let mut server_line = String::new();
    let mut session_token: Option<String> = None;
    loop {
        input_line.clear();
        print!("> ");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        let n = input.read_line(&mut input_line).await?;
        if n == 0 { break; }
        let cmd = input_line.trim();
        if cmd.is_empty() { continue; }
        let mut parts = cmd.split_whitespace();
        let command = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();
        // Comandi che NON richiedono session_token
        let public_cmds = ["/register", "/login", "/users", "/all_users"];
        // Comandi di messaggistica che richiedono token ma hanno parsing speciale
        let msg_cmds = ["/send", "/send_private", "/private", "/get_group_messages", "/get_private_messages", "/delete_group_messages", "/delete_private_messages"];
        let mut to_send = String::new();
        if public_cmds.contains(&command) {
            to_send = cmd.to_string();
        } else if msg_cmds.contains(&command) {
            if let Some(token) = &session_token {
                // Ricostruisci la sintassi server
                match command {
                    "/send" if args.len() >= 2 => {
                        let group = args[0];
                        let message = &args[1..].join(" ");
                        to_send = format!("/send {} {} {}", token, group, message);
                    }
                    "/send_private" | "/private" if args.len() >= 2 => {
                        let user = args[0];
                        let message = &args[1..].join(" ");
                        to_send = format!("{} {} {} {}", command, token, user, message);
                    }
                    "/get_group_messages" if args.len() == 1 => {
                        to_send = format!("/get_group_messages {} {}", token, args[0]);
                    }
                    "/get_private_messages" if args.len() == 1 => {
                        to_send = format!("/get_private_messages {} {}", token, args[0]);
                    }
                    "/delete_group_messages" if args.len() == 1 => {
                        to_send = format!("/delete_group_messages {} {}", token, args[0]);
                    }
                    "/delete_private_messages" if args.len() == 1 => {
                        to_send = format!("/delete_private_messages {} {}", token, args[0]);
                    }
                    _ => {
                        println!("[CLIENT] Sintassi comando non valida.");
                        continue;
                    }
                }
            } else {
                println!("[CLIENT] Devi prima effettuare il login!");
                continue;
            }
        } else {
            // Altri comandi autenticati generici
            if let Some(token) = &session_token {
                to_send = format!("{} {}{}", command, token, if args.is_empty() { "".to_string() } else { format!(" {}", args.join(" ")) });
            } else {
                println!("[CLIENT] Devi prima effettuare il login!");
                continue;
            }
        }
        server_writer.write_all(to_send.as_bytes()).await?;
        server_writer.write_all(b"\n").await?;
        server_writer.flush().await?;
        server_line.clear();
        let n = server_reader.read_line(&mut server_line).await?;
        if n == 0 {
            println!("[CLIENT] Server disconnesso");
            break;
        }
        let response = server_line.trim();
        println!("[SERVER] {}", response);
        // Estrai session_token dopo login
        if command == "/login" && response.contains("SESSION:") {
            if let Some(line) = response.lines().find(|l| l.contains("SESSION:")) {
                if let Some(token) = line.split("SESSION:").nth(1) {
                    session_token = Some(token.trim().to_string());
                    println!("[CLIENT] Login effettuato! Sessione attiva.");
                }
            }
        }
    }
    Ok(())
}

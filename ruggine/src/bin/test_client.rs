// Test client semplice per testare il server
use std::io::{self, Write, BufRead, BufReader};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to Ruggine server...");
    
    let stream = TcpStream::connect("127.0.0.1:5000")?;
    println!("Connected! Type commands or 'quit' to exit.");
    
    // Clona lo stream prima di usarlo
    let reader_stream = stream.try_clone()?;
    let mut writer = stream;
    
    // Thread per leggere dal server
    thread::spawn(move || {
        let mut reader = BufReader::new(reader_stream);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    println!("Server disconnected");
                    break;
                }
                Ok(_) => {
                    print!("Server: {}", line);
                    io::stdout().flush().unwrap();
                }
                Err(e) => {
                    println!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });
    
    // Aspetta un momento per ricevere il messaggio di benvenuto
    thread::sleep(Duration::from_millis(100));
    
    // Loop principale per input utente
    let stdin = io::stdin();
    loop {
        print!(">> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        stdin.read_line(&mut input)?;
        
        if input.trim() == "quit" {
            break;
        }
        
        writer.write_all(input.as_bytes())?;
        writer.flush()?;
    }
    
    Ok(())
}

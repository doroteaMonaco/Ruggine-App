__TARGET__
WINDOWS --> @echo off
REM build.bat
echo Building for Windows...
cargo build --release

echo Binary size:
dir target\release\ruggine*.exe



LINUX --> #!/bin/bash
# build.sh
echo "Building for current platform..."
cargo build --release

echo "Cross-compiling for Windows (if on Linux)..."
cargo build --release --target x86_64-pc-windows-gnu

echo "Binary sizes:"
ls -lh target/release/ruggine*

__DEPENDENCIES__
tokio - runtime asincrono per networking
serde + serde_json - serializzazione messaggi
clap - parsing argomenti command line
log + env_logger - sistema di logging
crossterm o tui-rs - interfaccia utente terminale
uuid - generazione ID univoci
chrono - gestione timestamp
sysinfo - monitoraggio CPU

__TESTS__
# Compila e avvia il server
cargo run --bin ruggine-server

# In un altro terminale, connettiti con telnet
telnet 127.0.0.1 5000

# Oppure su Windows
telnet localhost 5000


/register alice
/users
/help
/quit
<p align="center">
	<img src="./img/ruggineImage.png" alt="Ruggine Chat Application" width="420" />
</p>

# Ruggine 🦀 — Real-Time Chat Application

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)]()

**Ruggine** is a modern, secure, and scalable real-time chat application built entirely in Rust. It features end-to-end encryption, WebSocket-based real-time messaging, and a beautiful cross-platform desktop GUI. Designed for both educational purposes and production deployment.

## 🌟 Features

### 💬 **Real-Time Messaging**
- **Instant communication** via WebSocket connections
- **Private and group chats** with unlimited participants
- **Message persistence** with SQLite/PostgreSQL backend
- **Online presence tracking** with real-time user status
- **Message history** with full-text search capabilities

### 🔒 **Security & Privacy**
- **End-to-end encryption** using AES-256-GCM
- **Secure session management** with token-based authentication
- **TLS support** for production deployments
- **Password hashing** with industry-standard algorithms
- **Session timeout** and automatic cleanup

### 🚀 **Performance & Scalability**
- **Redis integration** for high-performance caching and pub/sub
- **Async architecture** built on Tokio for maximum concurrency
- **Connection pooling** and efficient resource management
- **Horizontal scaling** support with multiple server instances

### 🖥️ **Cross-Platform GUI**
- **Native desktop application** using Iced framework
- **Clean, modern interface** with dark/light theme support
- **Responsive design** that works on various screen sizes
- **Real-time notifications** for new messages
- **Multi-window support** for different conversations

### 🛠️ **Developer Experience**
- **Modular architecture** with clear separation of concerns
- **Comprehensive documentation** and code examples
- **Built-in monitoring** and performance metrics
- **Docker support** for easy deployment

## 📋 Table of Contents

- [Quick Start](#-quick-start)
- [Architecture](#-architecture)
- [Installation](#-installation)
- [Configuration](#-configuration)
- [Usage](#-usage)
- [Development](#-development)
- [Deployment](#-deployment)
- [API Documentation](#-api-documentation)
- [Contributing](#-contributing)
- [License](#-license)

## ⚡ Quick Start

### Prerequisites
- **Rust** 1.75+ (stable toolchain)
- **Redis** 6.0+ for real-time messaging
- **SQLite** (included) or **PostgreSQL** for data persistence

### 1. Clone and Build
```bash
git clone https://github.com/doroteaMonaco/Ruggine-App.git
cd Ruggine-App
cargo build --release
ENCRYPTION_MASTER_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
TLS_CERT_PATH=/etc/ssl/certs/ruggine.crt
TLS_KEY_PATH=/etc/ssl/private/ruggine.key
LOG_LEVEL=info
```

### 2. Setup Redis
```bash
# Install Redis locally
# Windows (with Chocolatey):
choco install redis-64

# macOS (with Homebrew):
brew install redis

# Linux (Ubuntu/Debian):
sudo apt install redis-server

# Start Redis
redis-server
```

### 3. Run the Application
```bash
# Terminal 1: Start the server
cargo run --bin ruggine-server

# Terminal 2: Start the GUI client
cargo run --bin ruggine-gui
```

That's it! You now have a fully functional chat application running locally.

## 🏗️ Architecture

Ruggine follows a modern client-server architecture designed for scalability and maintainability:

```
┌─────────────────┐    WebSocket    ┌─────────────────┐    Redis     ┌─────────────┐
│                 │   Connection    │                 │   Pub/Sub    │             │
│  Iced GUI       │◄───────────────►│  Rust Server    │◄────────────►│   Redis     │
│  Client         │                 │  (Tokio async)  │              │   Cache     │
│                 │                 │                 │              │             │
└─────────────────┘                 └─────────────────┘              └─────────────┘
                                              │                              
                                              │ SQLx                         
                                              ▼                              
                                    ┌─────────────────┐                      
                                    │                 │                      
                                    │ SQLite/Postgres │                      
                                    │   Database      │                      
                                    │                 │                      
                                    └─────────────────┘                      
```

### Key Components

- **🖥️ GUI Client**: Built with Iced framework for cross-platform native performance
- **⚡ Server**: Async Rust server using Tokio for handling thousands of concurrent connections
- **🔄 WebSocket Layer**: Real-time bidirectional communication between clients and server
- **📊 Redis**: High-performance caching and pub/sub for scaling across multiple instances
- **💾 Database**: Persistent storage with support for both SQLite (development) and PostgreSQL (production)

## 📦 Installation

### System Requirements
- **OS**: Windows 10+, macOS 10.15+, or Linux (Ubuntu 18.04+)
- **RAM**: 4GB minimum, 8GB recommended
- **Storage**: 500MB for application + database storage
- **Network**: Internet connection for initial setup

### Build from Source

1. **Install Rust** (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

2. **Clone the repository**:
```bash
git clone https://github.com/doroteaMonaco/Ruggine-App.git
cd Ruggine-App
```

3. **Install dependencies**:
```bash
# On Ubuntu/Debian
sudo apt update
sudo apt install build-essential pkg-config libssl-dev

# On macOS (with Homebrew)
brew install openssl pkg-config

# On Windows (with vcpkg)
vcpkg install openssl:x64-windows
```

4. **Build the project**:
```bash
cargo build --release
```

## ⚙️ Configuration

### Environment Variables

Ruggine uses environment variables for configuration. Create a `.env` file in the project root:

```bash
# Database Configuration
DATABASE_URL=sqlite:data/ruggine_modulare.db
# For PostgreSQL: DATABASE_URL=postgres://user:password@localhost/ruggine

# Redis Configuration  
REDIS_URL=redis://127.0.0.1:6379

# Server Configuration
SERVER_HOST=127.0.0.1
HTTP_PORT=5000
WEBSOCKET_PORT=5001

# Security
ENABLE_ENCRYPTION=true
ENCRYPTION_MASTER_KEY=your-32-byte-hex-key-here
SESSION_TIMEOUT_HOURS=24

# Logging
LOG_LEVEL=info
RUST_LOG=ruggine=debug

# TLS (for production)
TLS_CERT_PATH=/path/to/cert.pem
TLS_KEY_PATH=/path/to/key.pem
```

### Configuration Files

- `redis.conf`: Redis server configuration
- `Cargo.toml`: Rust dependencies and project metadata
- `docker-compose.yml`: Container orchestration setup

## 🚀 Usage

### Starting the Server

The server provides both HTTP and WebSocket endpoints:

```bash
# Development mode
cargo run --bin ruggine-server

# Production mode with release optimizations
cargo run --release --bin ruggine-server
```

Server will start on:
- **HTTP API**: `http://localhost:5000`
- **WebSocket**: `ws://localhost:5001`

### Client Applications

#### Desktop GUI
```bash
cargo run --bin ruggine-gui
```

Features:
- Modern, responsive interface
- Real-time message updates
- Contact management
- Group chat creation
- File sharing capabilities


### Basic Operations

1. **Register a new user**:
   - Launch the GUI client
   - Click "Register" 
   - Enter username and password
   - Click "Create Account"

2. **Login**:
   - Enter your credentials
   - Click "Login"

3. **Start chatting**:
   - Select a contact or create a group
   - Type your message
   - Press Enter to send

## 🔧 Development

### Project Structure

```
src/
├── bin/                    # Executable binaries
│   ├── ruggine-server.rs  # Main server application  
│   ├── ruggine-gui.rs     # Desktop GUI client
│   └── db_inspect.rs      # Database inspection utility
├── server/                 # Server-side components
│   ├── main.rs            # Server entry point
│   ├── websocket.rs       # WebSocket handling
│   ├── auth.rs            # Authentication & sessions
│   ├── database.rs        # Database operations
│   ├── messages.rs        # Message processing
│   ├── groups.rs          # Group chat management
│   └── redis_cache.rs     # Redis integration
├── client/                 # Client-side components  
│   ├── gui/               # Iced GUI components
│   │   ├── app.rs         # Main application state
│   │   ├── views/         # UI views and layouts
│   │   └── widgets/       # Custom UI widgets
│   ├── services/          # Client services
│   │   ├── websocket_client.rs  # WebSocket client
│   │   ├── chat_service.rs      # Chat operations
│   │   └── connection.rs        # Connection management
│   └── models/            # Data models
├── common/                 # Shared components
│   ├── models.rs          # Common data structures
│   └── crypto.rs          # Encryption utilities
└── utils/                 # Utility functions
    └── performance.rs     # Performance monitoring
```

### Adding New Features

1. **Create a feature branch**:
```bash
git checkout -b feature/your-feature-name
```

2. **Implement your changes** following the existing code patterns



4. **Update documentation** if needed

5. **Submit a pull request**

### Code Style

We follow standard Rust conventions:
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting  
- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

## 🚢 Deployment

### Docker Deployment

1. **Build Docker image**:
```bash
docker build -t ruggine-chat .
```

2. **Run with Docker Compose**:
```bash
docker-compose up -d
```

This starts:
- Ruggine server
- Redis instance  
- PostgreSQL database (if configured)

### Production Deployment

For production environments:

1. **Use PostgreSQL** instead of SQLite
2. **Enable TLS** with valid certificates
3. **Configure proper logging**
4. **Set up monitoring and alerting**
5. **Implement backup strategies**

Example production configuration:
```yaml
# docker-compose.prod.yml
version: '3.8'
services:
  ruggine:
    image: ruggine-chat:latest
    environment:
      - DATABASE_URL=postgres://user:pass@postgres:5432/ruggine
      - REDIS_URL=redis://redis:6379
      - ENABLE_TLS=true
      - TLS_CERT_PATH=/certs/fullchain.pem
      - TLS_KEY_PATH=/certs/privkey.pem
    volumes:
      - ./certs:/certs:ro
    ports:
      - "443:5000"
      - "5001:5001"
```

### Scaling

For high-traffic deployments:
- Use Redis cluster for horizontal scaling
- Deploy multiple server instances behind a load balancer
- Implement database read replicas
- Use CDN for static assets

## 📚 API Documentation

### WebSocket API

#### Authentication
```json
{
  "message_type": "auth",
  "session_token": "your-session-token-here"
}
```

#### Send Message
```json
{
  "message_type": "send_message", 
  "chat_type": "private",
  "to_user": "recipient_username",
  "content": "Hello, world!",
  "session_token": "your-session-token"
}
```

#### Group Messages
```json
{
  "message_type": "send_message",
  "chat_type": "group", 
  "group_id": "group_uuid",
  "content": "Hello everyone!",
  "session_token": "your-session-token"
}
```

### HTTP API

- `POST /register` - Register new user
- `POST /login` - User authentication  
- `POST /logout` - End user session
- `GET /users` - List online users
- `POST /groups` - Create group chat

### Security Features

- **End-to-end encryption** using AES-256-GCM
- **Secure session management** with automatic timeout
- **Input validation** and SQL injection prevention
- **Rate limiting** to prevent abuse
- **Audit logging** for security events

### Performance Optimizations

- **Connection pooling** for database operations
- **Redis caching** for frequently accessed data
- **WebSocket multiplexing** for efficient real-time communication
- **Async I/O** throughout the application stack
- **Zero-copy serialization** where possible

## 🤝 Contributing

We welcome contributions! Here's how you can help:

### Getting Started
1. Fork the repository
2. Create a feature branch
3. Make your changes
7. Submit a pull request

### Contribution Guidelines
- Follow the existing code style
- Write clear commit messages
- Add documentation for new features
- Ensure backward compatibility
- Update the changelog

### Areas for Contribution
- 🐛 Bug fixes and improvements
- ✨ New features and enhancements  
- 📖 Documentation improvements
- 🔧 Performance optimizations
- 🌍 Internationalization

### Code of Conduct
This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🔗 Links

- **Repository**: https://github.com/doroteaMonaco/Ruggine-App
- **Documentation**: [/doc](./doc) folder
- **Issues**: https://github.com/doroteaMonaco/Ruggine-App/issues
- **Rust**: https://www.rust-lang.org/
- **Iced GUI**: https://iced.rs/

## 🙏 Acknowledgments

- Built with ❤️ using the amazing Rust ecosystem
- Special thanks to the Iced GUI framework team
- Inspired by modern chat applications and Rust best practices
- Created by **Dorotea Monaco** and **Luigi Gonnella** as part of a distributed systems project

---

**Made with 🦀 Rust** | **Real-time** | **Secure** | **Cross-platform**
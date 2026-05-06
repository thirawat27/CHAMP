<div align="center">

<img src="src/assets/CHAMP.png" alt="CHAMP Logo" width="250"/>

## **CHAMP By Thirawat27**

*A modern, cross-platform local web development stack*

**C**addy + **H**TTP(S) + **A**dminer / phpmy**A**dmin + **M**ySQL + **P**HP

[Features](#-features) • [Installation](#-installation) • [Usage](#-usage) • [Development](#-development) • [Configuration](#-configuration)

</div>

---

## 📖 Overview

CHAMP is a self-contained desktop application that provides a complete local web development environment. Unlike traditional solutions like XAMPP, CHAMP is designed to run **without administrator privileges** by keeping all runtime binaries, configurations, logs, and data in the user's writable app data folder.

> **Note:** This project is a fork of [CAMPP](https://github.com/KarnYong/campp) by KarnYong, with enhancements and modifications by Thirawat27.

**Key Highlights:**
- 🚀 **No Admin Required** - Runs entirely in user space with non-default ports
- 📦 **Self-Contained** - All binaries bundled, no external dependencies
- 🎯 **Cross-Platform** - Windows, macOS, and Linux support
- 🔄 **Version Management** - Switch between multiple PHP and MySQL versions
- 🎨 **Modern UI** - Built with React and Tauri for a native experience
- ⚡ **Fast & Lightweight** - Rust-powered backend for optimal performance

---

## 🛠️ Tech Stack

### Current Versions

| Component   | Version | Description                    |
| ----------- | ------- | ------------------------------ |
| **Caddy**   | 2.11.2  | Modern web server with HTTPS   |
| **PHP**     | 8.5.5   | Latest PHP runtime (switchable)|
| **MySQL**   | 9.7.0   | LTS database server            |
| **phpMyAdmin** | 5.2.3 | Database management interface |

### Alternative Options

- **PHP**: 7.4.33 (EOL), 8.5.5 (Latest)
- **Database UI**: phpMyAdmin 5.2.3 or Adminer 5.4.2

---

## ✨ Features

### Core Functionality
- ✅ **One-Click Service Management** - Start/stop/restart all services with a single click
- ✅ **System Tray Integration** - Minimize to tray and control services from the system tray
- ✅ **Auto-Start Services** - Optionally start services automatically on app launch
- ✅ **Real-Time Status Monitoring** - Live service status updates and system metrics
- ✅ **Port Configuration** - Customize ports to avoid conflicts
- ✅ **Project Management** - Organize and access your web projects easily

### Advanced Features
- 🔄 **Multi-Version Support** - Install and switch between different PHP/MySQL versions
- 📦 **Package Selection** - Choose which components to install during first run
- 🔒 **Secure by Default** - Isolated user environment, no system-wide changes
- 📊 **System Metrics** - Monitor CPU, memory, and disk usage
- 🗂️ **Custom Configuration** - Advanced users can customize runtime configs
- 🐛 **Debug Mode** - Developer menu with runtime folder access and reset options

---

## 🌐 Default URLs

| Service          | URL                                  |
| ---------------- | ------------------------------------ |
| **Web Server**   | http://localhost:8080                |
| **phpMyAdmin**   | http://localhost:8080/phpmyadmin     |
| **Adminer**      | http://localhost:8080/adminer        |
| **MySQL**        | 127.0.0.1:3307                       |
| **PHP-FPM**      | 127.0.0.1:9000 (internal)            |

> **Note:** All ports can be customized in the Settings panel to avoid conflicts with other services.

---

## 📥 Installation

### Download Pre-Built Binaries

Download the latest release for your platform from the [Releases](https://github.com/thirawat27/CHAMP/releases) page:

- **Windows**: `CHAMP_1.1.0_x64_en-US.msi` or `.exe`
- **macOS**: `CHAMP_1.1.0_aarch64.dmg` (Apple Silicon) or `CHAMP_1.1.0_x64.dmg` (Intel)
- **Linux**: `CHAMP_1.1.0_amd64.AppImage` or `.deb`

### First Run Setup

1. Launch CHAMP
2. The **First-Run Wizard** will guide you through:
   - System dependency checks
   - Package selection (choose which components to install)
   - Runtime binary downloads
   - Initial configuration
3. Once complete, you're ready to start developing!

---

## 🚀 Usage

### Starting Services

1. Open CHAMP
2. Click **Start All** to launch all services
3. Access your projects at http://localhost:8080
4. Manage your database at http://localhost:8080/phpmyadmin

### Managing Services

- **Individual Control**: Start/stop/restart each service independently
- **Bulk Operations**: Use "Start All" or "Stop All" for convenience
- **Auto-Start**: Enable in Settings to start services on app launch

### System Tray

- **Minimize to Tray**: Close the window to minimize to system tray
- **Quick Access**: Right-click the tray icon for quick actions
- **Background Operation**: Services continue running when minimized

### Switching PHP Versions

1. Open **Settings** panel
2. Navigate to **PHP Version** section
3. Select desired version from dropdown
4. Click **Switch Version** (downloads if not installed)
5. Restart PHP-FPM service

---

## 💻 Development

### Prerequisites

- **Node.js** 18+ and **pnpm** 10+
- **Rust** 1.70+ (for Tauri backend)
- **Platform-specific requirements**:
  - Windows: WebView2 Runtime
  - macOS: Xcode Command Line Tools
  - Linux: webkit2gtk, libssl-dev

### Setup

```bash
# Clone the repository
git clone https://github.com/thirawat27/CHAMP.git
cd CHAMP

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev
```

### Available Scripts

#### Frontend (React + Vite)

```bash
pnpm dev          # Start Vite dev server (http://localhost:1420)
pnpm build        # Build frontend for production
pnpm preview      # Preview production build
pnpm test         # Run tests with Vitest
pnpm test:ui      # Run tests with UI
pnpm lint         # Lint TypeScript files
pnpm lint:fix     # Fix linting issues
pnpm format       # Format code with Prettier
```

#### Backend (Rust + Tauri)

```bash
cd src-tauri

cargo build       # Build Rust backend
cargo test        # Run Rust tests
cargo clippy      # Lint Rust code
cargo fmt         # Format Rust code
```

#### Full Application

```bash
pnpm tauri dev    # Run full app in dev mode
pnpm tauri build  # Build production app with installers
```

### Project Structure

```
CHAMP/
├── src/                      # React frontend
│   ├── components/           # UI components
│   │   ├── Dashboard.tsx     # Main dashboard
│   │   ├── ServiceCard.tsx   # Service control cards
│   │   ├── FirstRunWizard.tsx # Setup wizard
│   │   └── SettingsPanel.tsx # Settings UI
│   ├── hooks/                # Custom React hooks
│   ├── types/                # TypeScript definitions
│   └── App.tsx               # Main app component
├── src-tauri/                # Rust backend
│   ├── src/
│   │   ├── commands.rs       # Tauri IPC commands
│   │   ├── process/          # Service process management
│   │   ├── config/           # Configuration generation
│   │   ├── runtime/          # Binary download & locator
│   │   └── lib.rs            # Main library entry
│   ├── runtime-config.json   # Runtime binary definitions
│   └── tauri.conf.json       # Tauri app configuration
└── package.json              # Node.js dependencies
```

### Architecture

**Frontend (React + TypeScript)**
- Modern React 19 with TypeScript
- Tailwind CSS for styling
- Lucide React for icons
- Tauri IPC for backend communication

**Backend (Rust + Tauri)**
- Modular architecture with clear separation of concerns
- Process management for service lifecycle
- Configuration generation with Mustache templates
- Runtime binary download and verification
- Cross-platform path handling

---

## ⚙️ Configuration

### User Data Directory

CHAMP stores all data in platform-specific user directories:

**Windows:**
```
%LOCALAPPDATA%\CHAMP\
├── config\           # Generated service configs
├── logs\             # Service logs
├── mysql\data\       # MySQL database files
├── projects\         # Your web projects
└── runtime\          # Downloaded binaries
```

**macOS/Linux:**
```
~/.campp/
├── config/
├── logs/
├── mysql/data/
├── projects/
└── runtime/
```

### Runtime Configuration

Advanced users can customize binary versions and download URLs by editing:
- `runtime-config.json` - Default configuration (bundled with app)
- `runtime-config.user.json` - User overrides (optional)

See `runtime-config.schema.json` for the full configuration schema.

### Settings

Accessible via the Settings panel in the app:
- **Ports**: Customize service ports
- **Auto-Start**: Enable/disable automatic service startup
- **Project Folder**: Set default project directory
- **PHP Version**: Switch between installed PHP versions
- **MySQL Version**: Switch between installed MySQL versions

---

## 🐛 Troubleshooting

### Services Won't Start

1. Check if ports are already in use
2. Review service logs in `logs/` directory
3. Verify runtime binaries are installed
4. Try "Reset Installation" from Debug menu (dev mode)

### Port Conflicts

1. Open Settings panel
2. Change conflicting ports
3. Restart affected services

### Reset Installation

In development mode:
1. Open Debug menu
2. Select "Reset Installation"
3. Re-run First-Run Wizard

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

**License Holder:** Thirawat Sinlapasomsak

---

## 🙏 Acknowledgments

- **Original Project**: [CAMPP](https://github.com/KarnYong/campp) by KarnYong - The foundation and inspiration for this project
- [Tauri](https://tauri.app/) - Desktop app framework
- [Caddy](https://caddyserver.com/) - Modern web server
- [PHP](https://www.php.net/) - Server-side scripting
- [MySQL](https://www.mysql.com/) - Database server
- [phpMyAdmin](https://www.phpmyadmin.net/) - Database management
- [React](https://react.dev/) - UI framework

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/thirawat27/CHAMP/issues)
- **Repository**: [github.com/thirawat27/CHAMP](https://github.com/thirawat27/CHAMP)

---

<div align="center">

Made with ❤️ by [Thirawat27](https://github.com/thirawat27)

</div>

# Zonky Desktop UI

Tauri v2 + Svelte 5 desktop application for the Zonky LLM inference engine.

## Prerequisites

### System Libraries (Ubuntu/Debian)

```bash
sudo apt install -y libwebkit2gtk-4.1-dev librsvg2-dev
```

This pulls in all required GTK/WebKit development libraries (libgtk-3-dev, libsoup-3.0-dev, libjavascriptcoregtk-4.1-dev, etc.)

### Node.js

```bash
# Using nvm (recommended)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
nvm install --lts
```

### Tauri CLI

```bash
npm install -g @tauri-apps/cli@latest
```

## Development

```bash
# From the workspace root
cd crates/zonky-ui
cargo tauri dev
```

This starts the Vite dev server and the Tauri app with hot reload.

### Windows Shortcut

From the workspace root you can also run:

```powershell
powershell -ExecutionPolicy Bypass -File .\run-ui.ps1
```

This script starts Vite (if needed) and launches `zonky-ui`.

## Production Build

```bash
cd crates/zonky-ui
cargo tauri build
```

## Structure

```
crates/zonky-ui/
├── Cargo.toml          # Rust dependencies (Tauri backend)
├── build.rs            # Tauri build script
├── tauri.conf.json     # Tauri configuration
├── capabilities/       # Tauri permissions
├── src/
│   ├── main.rs         # Tauri app entry point
│   └── commands.rs     # IPC command handlers
└── frontend/
    ├── package.json    # Node.js dependencies
    ├── vite.config.js  # Vite configuration
    ├── index.html      # HTML entry point
    └── src/
        ├── main.js     # Svelte mount
        ├── App.svelte  # Main layout + sidebar
        └── routes/
            ├── Dashboard.svelte  # GPU, memory, setup status
            ├── Models.svelte     # Local models + HuggingFace Hub
            ├── Chat.svelte       # Chat interface
            └── Settings.svelte   # Configuration display
```

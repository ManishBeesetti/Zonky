# Zonky

Local-first LLM workspace with:
- `zonky-core` inference and model orchestration
- `zonky-server` OpenAI-compatible API
- `zonky-cli` command-line workflows
- `zonky-tui` terminal UI
- `zonky-ui` desktop UI (Tauri + Svelte)

## Windows Prerequisites

Install these before building:

1. Rust toolchain
```powershell
winget install Rustlang.Rustup
```

2. Visual Studio 2022 Build Tools (C++ workload)
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```
Required components:
- Desktop development with C++
- MSVC v143 toolset
- Windows 10/11 SDK

3. CMake
```powershell
winget install Kitware.CMake
```

4. Node.js LTS (includes npm)
```powershell
winget install OpenJS.NodeJS.LTS
```

5. Playwright browser (for UI tests)
```powershell
cd crates/zonky-ui/frontend
npx playwright install chromium
```

## Fast Windows Check

From repo root:

```powershell
.\run-ui.ps1 -CheckOnly
```

This verifies:
- Rust (`cargo` or `rustup`)
- MSVC Build Tools
- `cmake` in PATH
- `node` and `npm`
- no running `zonky-ui.exe` lock conflict

## Run Desktop UI

```powershell
.\run-ui.ps1 -KillExisting
```

## Build and Test

From repo root:

```powershell
cargo check --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

If `cargo` is unavailable in your shell:

```powershell
rustup run stable cargo check --workspace
```

Frontend checks:

```powershell
cd crates/zonky-ui/frontend
npm install
npm run build
npx playwright test --project=chromium
```

## Troubleshooting

- `is cmake not installed?`:
  Ensure `cmake --version` works in the same shell.
- `Access is denied` for `zonky-ui.exe`:
  Close the app or run:

```powershell
.\run-ui.ps1 -KillExisting
```

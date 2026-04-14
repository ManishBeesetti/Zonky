# Zonky

## Run UI (Windows)

Use the helper script from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\run-ui.ps1
```

The script will:
- install frontend dependencies if needed
- start the Vite dev server on `127.0.0.1:5173` if not already running
- launch `zonky-ui`

If startup fails, check:
- `crates/zonky-ui/frontend/vite.out.log`
- `crates/zonky-ui/frontend/vite.err.log`

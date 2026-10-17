// Shared hardware state — polls Tauri backend for live GPU/model/server data

/** @type {{ vramUsed: number, vramFree: number, vramTotal: number, vramPercent: number, gpuName: string, gpuVendor: string, loadedModels: any[], localModels: any[], serverStatus: { running: boolean, host: string, port: number, url: string }, totalMemory: number, availableMemory: number }} */
let hw = $state({
  vramUsed: 0,
  vramFree: 0,
  vramTotal: 0,
  vramPercent: 0,
  gpuName: "No GPU",
  gpuVendor: "",
  loadedModels: [],
  localModels: [],
  serverStatus: {
    running: false,
    host: "127.0.0.1",
    port: 8080,
    url: "http://127.0.0.1:8080",
  },
  totalMemory: 0,
  availableMemory: 0,
});

let polling = false;
let inFlight = false;
let intervalId = null;

export function getHardware() {
  return hw;
}

export function setServerStatus(status) {
  if (status) {
    hw.serverStatus = status;
  }
}

async function poll() {
  if (inFlight) return;
  inFlight = true;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const [sysInfo, loaded, local, srvStatus] = await Promise.all([
      invoke("get_system_info").catch(() => null),
      invoke("list_loaded_models").catch(() => null),
      invoke("list_local_models").catch(() => null),
      invoke("get_server_status").catch(() => null),
    ]);

    if (sysInfo) {
      const gpu = sysInfo.devices?.find((d) => d.is_gpu);
      if (gpu) {
        hw.vramTotal = gpu.vram_total;
        hw.vramFree = gpu.vram_free;
        hw.vramUsed = Math.max(0, gpu.vram_total - gpu.vram_free);
        hw.vramPercent =
          gpu.vram_total > 0
            ? Math.round((hw.vramUsed / gpu.vram_total) * 100)
            : 0;
        hw.gpuName = gpu.name;
        hw.gpuVendor = gpu.vendor;
      }

      hw.totalMemory = sysInfo.total_memory || 0;
      hw.availableMemory = sysInfo.available_memory || 0;
    }

    if (Array.isArray(loaded)) {
      hw.loadedModels = loaded;
    }
    if (Array.isArray(local)) {
      hw.localModels = local;
    }
    if (srvStatus && typeof srvStatus.running === "boolean") {
      hw.serverStatus = srvStatus;
    }
  } catch (_) {
    // Silently fail during polling — don't break the UI
  } finally {
    inFlight = false;
  }
}

/** Start polling if not already running */
export function startPolling(intervalMs = 3000) {
  if (polling) return;
  polling = true;
  // Immediate first fetch
  poll();
  intervalId = setInterval(poll, intervalMs);
}

/** Stop polling (e.g. on app teardown) */
export function stopPolling() {
  if (intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }
  polling = false;
}

/** Force a single refresh */
export async function refresh() {
  inFlight = false;
  await poll();
}

// Shared hardware state — polls Tauri backend for live GPU/model data

/** @type {{ vramUsed: number, vramFree: number, vramTotal: number, vramPercent: number, gpuName: string, gpuVendor: string, loadedModels: any[], totalMemory: number, availableMemory: number }} */
let hw = $state({
  vramUsed: 0,
  vramFree: 0,
  vramTotal: 0,
  vramPercent: 0,
  gpuName: "No GPU",
  gpuVendor: "",
  loadedModels: [],
  totalMemory: 0,
  availableMemory: 0,
});

let polling = false;
let intervalId = null;

export function getHardware() {
  return hw;
}

async function poll() {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const [sysInfo, loaded] = await Promise.all([
      invoke("get_system_info"),
      invoke("list_loaded_models"),
    ]);

    const gpu = sysInfo.devices?.find((d) => d.is_gpu);
    if (gpu) {
      hw.vramTotal = gpu.vram_total;
      hw.vramFree = gpu.vram_free;
      hw.vramUsed = gpu.vram_total - gpu.vram_free;
      hw.vramPercent =
        gpu.vram_total > 0
          ? Math.round(((gpu.vram_total - gpu.vram_free) / gpu.vram_total) * 100)
          : 0;
      hw.gpuName = gpu.name;
      hw.gpuVendor = gpu.vendor;
    }

    hw.totalMemory = sysInfo.total_memory || 0;
    hw.availableMemory = sysInfo.available_memory || 0;
    hw.loadedModels = loaded || [];
  } catch (_) {
    // Silently fail during polling — don't break the UI
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
  await poll();
}

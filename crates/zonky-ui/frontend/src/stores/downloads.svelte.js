// Shared download state — reactive Svelte 5 runes at module level

const STORAGE_KEY = "zonky_downloads_v1";

function loadPersistedDownloads() {
  if (typeof window === "undefined" || !window.localStorage) return [];
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.map((d) => {
      if (d.status === "downloading" || d.status === "queued") {
        return {
          ...d,
          status: "error",
          speed: 0,
          error: "Download interrupted — click Retry to resume",
        };
      }
      return d;
    });
  } catch (_) {
    return [];
  }
}

function persistDownloads(list) {
  if (typeof window === "undefined" || !window.localStorage) return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch (_) {}
}

/** @type {{ id: string, repoId: string, filename: string, size: number|null, downloaded: number, speed: number, status: 'queued'|'downloading'|'done'|'error', error?: string, startedAt: number }[]} */
let downloads = $state(loadPersistedDownloads());
let listenerActive = false;

export function getDownloads() {
  return downloads;
}

export function activeCount() {
  return downloads.filter((d) => d.status === "downloading" || d.status === "queued").length;
}

/** Start listening for backend progress events */
async function ensureListener() {
  if (listenerActive) return;
  listenerActive = true;
  try {
    const { listen } = await import("@tauri-apps/api/event");
    await listen("download-progress", (event) => {
      const { id, downloaded, total, speed } = event.payload;
      downloads = downloads.map((d) => {
        if (d.id !== id) return d;
        return {
          ...d,
          downloaded: downloaded || d.downloaded,
          size: total || d.size,
          speed: speed || 0,
          status: "downloading",
        };
      });
      persistDownloads(downloads);
    });
  } catch (_) {
    // Not in Tauri environment (tests, etc.)
  }
}

export function addDownload(repoId, filename, size) {
  const id = `${repoId}/${filename}`;
  // Don't duplicate active downloads
  if (downloads.find((d) => d.id === id && (d.status === "queued" || d.status === "downloading")))
    return;
  // Remove previous failed/done entry with same id if re-queued
  const filtered = downloads.filter((d) => d.id !== id);
  downloads = [
    {
      id,
      repoId,
      filename,
      size,
      downloaded: 0,
      speed: 0,
      status: "queued",
      startedAt: Date.now(),
    },
    ...filtered,
  ];
  persistDownloads(downloads);
  ensureListener();
  startDownload(id);
}

async function startDownload(id) {
  const dl = downloads.find((d) => d.id === id);
  if (!dl) return;

  downloads = downloads.map((d) =>
    d.id === id ? { ...d, status: "downloading", error: undefined } : d
  );
  persistDownloads(downloads);

  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("pull_model", { repoId: dl.repoId, filename: dl.filename });
    downloads = downloads.map((d) =>
      d.id === id ? { ...d, status: "done", downloaded: d.size || d.downloaded, speed: 0 } : d
    );
    persistDownloads(downloads);
  } catch (e) {
    downloads = downloads.map((d) =>
      d.id === id ? { ...d, status: "error", error: String(e), speed: 0 } : d
    );
    persistDownloads(downloads);
  }
}

export async function cancelDownload(id) {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("cancel_download", { id });
  } catch (_) {}
  downloads = downloads.map((d) =>
    d.id === id
      ? { ...d, status: "error", error: "Cancelled by user", speed: 0 }
      : d
  );
  persistDownloads(downloads);
}

export function retryDownload(id) {
  const dl = downloads.find((d) => d.id === id);
  if (!dl) return;
  downloads = downloads.map((d) =>
    d.id === id
      ? { ...d, status: "queued", downloaded: 0, speed: 0, error: undefined, startedAt: Date.now() }
      : d
  );
  persistDownloads(downloads);
  ensureListener();
  startDownload(id);
}

export function removeDownload(id) {
  downloads = downloads.filter((d) => d.id !== id);
  persistDownloads(downloads);
}

export function clearCompleted() {
  downloads = downloads.filter((d) => d.status !== "done" && d.status !== "error");
  persistDownloads(downloads);
}


// Shared download state — reactive Svelte 5 runes at module level

/** @type {{ id: string, repoId: string, filename: string, size: number|null, downloaded: number, speed: number, status: 'queued'|'downloading'|'done'|'error', error?: string, startedAt: number }[]} */
let downloads = $state([]);
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
    });
  } catch (_) {
    // Not in Tauri environment (tests, etc.)
  }
}

export function addDownload(repoId, filename, size) {
  const id = `${repoId}/${filename}`;
  // Don't duplicate
  if (downloads.find((d) => d.id === id && (d.status === "queued" || d.status === "downloading")))
    return;
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
    ...downloads,
  ];
  ensureListener();
  startDownload(id);
}

async function startDownload(id) {
  const dl = downloads.find((d) => d.id === id);
  if (!dl) return;

  downloads = downloads.map((d) =>
    d.id === id ? { ...d, status: "downloading" } : d
  );

  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("pull_model", { repoId: dl.repoId, filename: dl.filename });
    downloads = downloads.map((d) =>
      d.id === id ? { ...d, status: "done", downloaded: d.size || d.downloaded, speed: 0 } : d
    );
  } catch (e) {
    downloads = downloads.map((d) =>
      d.id === id ? { ...d, status: "error", error: String(e), speed: 0 } : d
    );
  }
}

export function removeDownload(id) {
  downloads = downloads.filter((d) => d.id !== id);
}

export function clearCompleted() {
  downloads = downloads.filter((d) => d.status !== "done" && d.status !== "error");
}

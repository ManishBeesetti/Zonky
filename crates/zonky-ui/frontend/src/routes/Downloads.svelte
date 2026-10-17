<script>
  import { getDownloads, removeDownload, clearCompleted } from "../stores/downloads.svelte.js";

  let downloads = $derived(getDownloads());
  let activeDownloads = $derived(downloads.filter((d) => d.status === "downloading" || d.status === "queued"));
  let completedDownloads = $derived(downloads.filter((d) => d.status === "done"));
  let failedDownloads = $derived(downloads.filter((d) => d.status === "error"));

  /** Format bytes: MB until 1024 MB, then GB with 0.1 increments */
  function formatSize(bytes) {
    if (!bytes || bytes === 0) return "0 MB";
    const mb = bytes / (1024 * 1024);
    if (mb < 1024) return mb.toFixed(0) + " MB";
    const gb = bytes / (1024 * 1024 * 1024);
    return gb.toFixed(1) + " GB";
  }

  function formatSpeed(bytesPerSec) {
    if (!bytesPerSec || bytesPerSec === 0) return "-";
    const mb = bytesPerSec / (1024 * 1024);
    return mb.toFixed(1) + " MB/s";
  }

  function progressPercent(dl) {
    if (!dl.size || dl.size === 0) return 0;
    return Math.min(100, Math.round((dl.downloaded / dl.size) * 100));
  }

  function timeAgo(ts) {
    const sec = Math.floor((Date.now() - ts) / 1000);
    if (sec < 60) return `${sec}s ago`;
    if (sec < 3600) return `${Math.floor(sec / 60)}m ago`;
    return `${Math.floor(sec / 3600)}h ago`;
  }
</script>

<div class="min-h-full px-8 py-10">
  <div class="max-w-4xl mx-auto space-y-8">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div class="space-y-2">
        <h1 class="text-4xl font-headline font-bold tracking-tighter text-on-surface">
          Downloads
        </h1>
        <p class="text-on-surface-variant text-sm">
          Track model downloads and manage your download queue.
        </p>
      </div>
      {#if completedDownloads.length > 0 || failedDownloads.length > 0}
        <button
          class="flex items-center gap-2 px-4 py-2 rounded-lg bg-surface-container-high text-on-surface-variant text-sm font-medium hover:text-primary hover:bg-primary/10 transition-all"
          onclick={clearCompleted}
        >
          <span class="material-symbols-outlined text-sm">delete_sweep</span>
          Clear History
        </button>
      {/if}
    </div>

    <!-- Active Downloads -->
    {#if activeDownloads.length > 0}
      <section class="space-y-3">
        <h2 class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold">
          Active Downloads ({activeDownloads.length})
        </h2>
        <div class="space-y-2">
          {#each activeDownloads as dl}
            <div class="bg-surface-container-lowest rounded-xl p-4 border border-tertiary/20">
              <div class="flex items-center gap-4">
                <div class="w-10 h-10 rounded-lg bg-tertiary/10 flex items-center justify-center flex-shrink-0">
                  {#if dl.status === "downloading"}
                    <span class="material-symbols-outlined text-tertiary animate-spin">progress_activity</span>
                  {:else}
                    <span class="material-symbols-outlined text-on-surface-variant">hourglass_top</span>
                  {/if}
                </div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-center justify-between mb-1">
                    <h3 class="text-sm font-headline font-bold text-on-surface truncate">{dl.filename}</h3>
                    <span class="text-[10px] font-mono text-tertiary uppercase ml-2 flex-shrink-0">
                      {dl.status === "downloading" ? `${progressPercent(dl)}%` : "Queued"}
                    </span>
                  </div>
                  <p class="text-xs text-on-surface-variant truncate">{dl.repoId}</p>
                  <div class="flex items-center gap-4 mt-2">
                    <span class="text-xs font-mono text-primary">
                      {formatSize(dl.downloaded)} / {formatSize(dl.size)}
                    </span>
                    {#if dl.speed > 0}
                      <span class="text-xs font-mono text-secondary">{formatSpeed(dl.speed)}</span>
                    {/if}
                    <span class="text-xs text-on-surface-variant/40 ml-auto">Started {timeAgo(dl.startedAt)}</span>
                  </div>
                  <!-- Live progress bar -->
                  <div class="mt-3 h-1.5 rounded-full bg-surface-container-high overflow-hidden">
                    <div
                      class="h-full bg-tertiary rounded-full transition-all duration-300"
                      style="width: {progressPercent(dl)}%"
                    ></div>
                  </div>
                </div>
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- Completed -->
    {#if completedDownloads.length > 0}
      <section class="space-y-3">
        <h2 class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold">
          Completed ({completedDownloads.length})
        </h2>
        <div class="space-y-2">
          {#each completedDownloads as dl}
            <div class="bg-surface-container-lowest rounded-xl p-4 border border-secondary/20">
              <div class="flex items-center gap-4">
                <div class="w-10 h-10 rounded-lg bg-secondary/10 flex items-center justify-center flex-shrink-0">
                  <span class="material-symbols-outlined text-secondary" style="font-variation-settings: 'FILL' 1;">check_circle</span>
                </div>
                <div class="flex-1 min-w-0">
                  <h3 class="text-sm font-headline font-bold text-on-surface truncate">{dl.filename}</h3>
                  <p class="text-xs text-on-surface-variant truncate">{dl.repoId}</p>
                  <div class="flex items-center gap-4 mt-1">
                    <span class="text-xs font-mono text-secondary">{formatSize(dl.size)}</span>
                    <span class="text-xs text-on-surface-variant/40">Completed</span>
                  </div>
                </div>
                <button
                  class="p-2 rounded-lg text-on-surface-variant hover:text-error hover:bg-error/10 transition-colors flex-shrink-0"
                  onclick={() => removeDownload(dl.id)}
                  title="Remove from list"
                >
                  <span class="material-symbols-outlined text-sm">close</span>
                </button>
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- Failed -->
    {#if failedDownloads.length > 0}
      <section class="space-y-3">
        <h2 class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold">
          Failed ({failedDownloads.length})
        </h2>
        <div class="space-y-2">
          {#each failedDownloads as dl}
            <div class="bg-surface-container-lowest rounded-xl p-4 border border-error/20">
              <div class="flex items-center gap-4">
                <div class="w-10 h-10 rounded-lg bg-error/10 flex items-center justify-center flex-shrink-0">
                  <span class="material-symbols-outlined text-error">error</span>
                </div>
                <div class="flex-1 min-w-0">
                  <h3 class="text-sm font-headline font-bold text-on-surface truncate">{dl.filename}</h3>
                  <p class="text-xs text-on-surface-variant truncate">{dl.repoId}</p>
                  <p class="text-xs text-error mt-1 truncate">{dl.error}</p>
                </div>
                <button
                  class="p-2 rounded-lg text-on-surface-variant hover:text-error hover:bg-error/10 transition-colors flex-shrink-0"
                  onclick={() => removeDownload(dl.id)}
                  title="Dismiss"
                >
                  <span class="material-symbols-outlined text-sm">close</span>
                </button>
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- Empty State -->
    {#if downloads.length === 0}
      <div class="flex flex-col items-center justify-center py-20 text-center">
        <div class="w-16 h-16 rounded-xl bg-surface-container-high flex items-center justify-center mb-6">
          <span class="material-symbols-outlined text-3xl text-on-surface-variant">cloud_download</span>
        </div>
        <h3 class="font-headline text-xl text-on-surface mb-2">No Downloads</h3>
        <p class="text-on-surface-variant text-sm max-w-md">
          Go to the Model Hub to search and download GGUF models for local inference.
        </p>
      </div>
    {/if}
  </div>
</div>

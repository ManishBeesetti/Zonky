<script>
  import { onMount } from "svelte";
  import { addDownload, getDownloads } from "../stores/downloads.svelte.js";

  let searchResults = $state([]);
  let searchQuery = $state("");
  let searching = $state(false);
  let pullStatus = $state("");
  let localModels = $state([]);
  let filePicker = $state(null); // { repoId, files, loading }
  let filterModelsOnly = $state(true);
  let allDownloads = $derived(getDownloads());

  const NON_MODEL_PATTERNS = ["mmproj", "vision-encoder", "clip-", "image-encoder", "visual"];
  function isNonModel(filename) {
    const lower = filename.toLowerCase();
    return NON_MODEL_PATTERNS.some((p) => lower.includes(p));
  }

  const trendingTags = ["Coding", "Uncensored", "RP", "Chat-Optimized", "Fine-tuned", "Reasoning"];

  const modelIcons = {
    llama: "smart_toy",
    mistral: "auto_awesome",
    phi: "bolt",
    code: "code",
    gemma: "psychology",
    default: "smart_toy",
  };

  const modelColors = {
    llama: "primary",
    mistral: "tertiary",
    phi: "primary",
    code: "on-surface-variant",
    gemma: "primary",
    default: "primary",
  };

  onMount(async () => {
    await loadLocalModels();
  });

  async function loadLocalModels() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      localModels = await invoke("list_local_models");
    } catch (_) {}
  }

  async function searchModels() {
    if (!searchQuery.trim()) return;
    searching = true;
    filePicker = null;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      searchResults = await invoke("search_models", {
        query: searchQuery,
        limit: 20,
      });
    } catch (e) {
      console.error("Search failed:", e);
    }
    searching = false;
  }

  async function showFilePicker(repoId) {
    if (filePicker?.repoId === repoId) {
      filePicker = null;
      return;
    }
    filePicker = { repoId, files: [], loading: true };
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const files = await invoke("list_gguf_files", { repoId });
      filePicker = { repoId, files, loading: false };
    } catch (e) {
      pullStatus = `Error listing files: ${e}`;
      filePicker = null;
    }
  }

  function pullModel(repoId, filename, size) {
    filePicker = null;
    addDownload(repoId, filename, size);
    pullStatus = `Queued ${filename} for download.`;
  }

  function handleSearchKey(e) {
    if (e.key === "Enter") searchModels();
  }

  function tagSearch(tag) {
    searchQuery = tag.toLowerCase();
    searchModels();
  }

  function getIcon(modelId) {
    const lower = (modelId || "").toLowerCase();
    for (const [key, icon] of Object.entries(modelIcons)) {
      if (key !== "default" && lower.includes(key)) return icon;
    }
    return modelIcons.default;
  }

  function getColor(modelId) {
    const lower = (modelId || "").toLowerCase();
    for (const [key, color] of Object.entries(modelColors)) {
      if (key !== "default" && lower.includes(key)) return color;
    }
    return modelColors.default;
  }

  function guessArch(modelId) {
    const lower = (modelId || "").toLowerCase();
    if (lower.includes("llama")) return "Llama Architecture";
    if (lower.includes("mistral")) return "Mistral Architecture";
    if (lower.includes("phi")) return "Phi Architecture";
    if (lower.includes("gemma")) return "Gemma Architecture";
    if (lower.includes("qwen")) return "Qwen Architecture";
    if (lower.includes("code") || lower.includes("starcoder")) return "Code Architecture";
    return "Transformer";
  }

  function isInstalled(modelId) {
    return localModels.some((m) => m.repo_id === modelId);
  }

  function formatDownloads(n) {
    if (!n) return "0";
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + "M";
    if (n >= 1_000) return (n / 1_000).toFixed(0) + "K";
    return n.toString();
  }

  function formatSize(bytes) {
    if (!bytes) return "—";
    const gb = bytes / (1024 * 1024 * 1024);
    if (gb >= 1) return gb.toFixed(1) + " GB";
    const mb = bytes / (1024 * 1024);
    return mb.toFixed(0) + " MB";
  }
</script>

<div class="min-h-full">
  <!-- Search & Filters Hero Section -->
  <section class="px-8 py-10" style="background: rgba(28, 27, 28, 0.3);">
    <div class="max-w-6xl mx-auto space-y-8">
      <div class="space-y-2">
        <h1 class="text-4xl font-headline font-bold tracking-tighter text-on-surface">
          Model Hub
        </h1>
        <p class="text-on-surface-variant text-sm max-w-xl">
          Browse optimized LLMs for local inference. Direct integration with HuggingFace Hub.
        </p>
      </div>

      <!-- Search Bar -->
      <div class="grid grid-cols-1 md:grid-cols-12 gap-4">
        <div class="md:col-span-8 relative">
          <span
            class="material-symbols-outlined absolute left-4 top-1/2 -translate-y-1/2 text-on-surface-variant/50"
            >search</span
          >
          <input
            class="w-full bg-surface-container-high border-none rounded-xl py-4 pl-12 pr-4 text-on-surface placeholder:text-on-surface-variant/40 focus-glow transition-all"
            placeholder="Search models by name or task..."
            type="text"
            bind:value={searchQuery}
            onkeydown={handleSearchKey}
          />
        </div>
        <div class="md:col-span-4">
          <button
            class="w-full bg-primary text-on-primary py-4 rounded-xl font-headline font-bold uppercase tracking-tight hover:shadow-[0_0_15px_rgba(76,214,255,0.4)] transition-all flex items-center justify-center gap-2 disabled:opacity-50"
            onclick={searchModels}
            disabled={searching || !searchQuery.trim()}
          >
            {#if searching}
              <span class="material-symbols-outlined text-sm animate-spin">progress_activity</span>
              Searching...
            {:else}
              <span class="material-symbols-outlined text-sm">search</span>
              Search Hub
            {/if}
          </button>
        </div>
      </div>

      <!-- Trending Tags -->
      <div class="flex items-center gap-2 overflow-x-auto no-scrollbar pb-2">
        <span class="text-[10px] font-bold uppercase tracking-widest text-on-surface-variant/50 mr-2 flex-shrink-0"
          >Trending Tags:</span
        >
        {#each trendingTags as tag}
          <button
            class="px-3 py-1 bg-surface-container-high rounded-2xl text-xs font-medium hover:bg-primary/20 hover:text-primary transition-colors flex-shrink-0"
            onclick={() => tagSearch(tag)}
          >
            {tag}
          </button>
        {/each}
      </div>
    </div>
  </section>

  <!-- Status Bar -->
  {#if pullStatus}
    <div class="mx-8 mt-4">
      <div
        class="max-w-6xl mx-auto glass-panel p-4 rounded-xl flex items-center gap-4 border-l-2 border-secondary"
      >
        <div
          class="w-8 h-8 rounded flex items-center justify-center bg-secondary/20 text-secondary"
        >
          <span class="material-symbols-outlined text-lg">check_circle</span>
        </div>
        <p class="text-sm font-mono text-on-surface-variant">{pullStatus}</p>
      </div>
    </div>
  {/if}

  <!-- Results Grid -->
  <section class="px-8 py-8 max-w-6xl mx-auto">
    {#if searchResults.length === 0 && !searching}
      <div
        class="flex flex-col items-center justify-center py-20 text-center"
      >
        <div
          class="w-16 h-16 rounded-xl bg-surface-container-high flex items-center justify-center mb-6"
        >
          <span class="material-symbols-outlined text-3xl text-on-surface-variant">hub</span>
        </div>
        <h3 class="font-headline text-xl text-on-surface mb-2">Search the Hub</h3>
        <p class="text-on-surface-variant text-sm max-w-md">
          Search for GGUF-quantized models optimized for local inference. Try "llama GGUF",
          "mistral", or click a trending tag.
        </p>
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {#each searchResults as result}
          {@const installed = isInstalled(result.model_id)}
          {@const downloading = allDownloads.some((d) => d.repoId === result.model_id && (d.status === 'downloading' || d.status === 'queued'))}
          {@const pickerOpen = filePicker?.repoId === result.model_id}
          <div
            class="bg-surface-container-lowest p-5 rounded-xl hover:bg-surface-container-low transition-all group relative overflow-hidden
            {installed
              ? 'border border-primary/20 bg-primary/5'
              : downloading
                ? 'border border-tertiary/20 bg-tertiary/5'
                : 'border border-outline-variant/10'}"
          >
            <!-- Card Header -->
            <div class="flex justify-between items-start mb-4">
              <div
                class="w-10 h-10 rounded-lg bg-{getColor(result.model_id)}/10 flex items-center justify-center text-{getColor(result.model_id)}"
              >
                <span class="material-symbols-outlined">{getIcon(result.model_id)}</span>
              </div>
              <div class="flex flex-col items-end gap-1">
                <span class="text-[10px] font-mono text-secondary uppercase tracking-widest"
                  >GGUF</span
                >
                <span class="text-[10px] text-on-surface-variant">{guessArch(result.model_id)}</span>
              </div>
            </div>

            <!-- Model Info -->
            <h3 class="text-lg font-headline font-bold mb-1 truncate text-on-surface">
              {result.model_id.split("/").pop()}
            </h3>
            <p class="text-sm text-on-surface-variant mb-4">
              {result.author || "Unknown"} · {formatDownloads(result.downloads)} Downloads
            </p>

            <!-- Stats Row -->
            <div class="flex items-center gap-4 mb-4">
              <div class="space-y-0.5">
                <span class="text-[10px] uppercase font-bold text-on-surface-variant/40 block"
                  >Likes</span
                >
                <span class="text-sm font-medium flex items-center gap-1">
                  <span class="material-symbols-outlined text-[14px] text-error">favorite</span>
                  {result.likes.toLocaleString()}
                </span>
              </div>
            </div>

            <!-- Action Buttons -->
            <div class="flex items-center gap-2 mt-auto">
              <!-- HuggingFace Link -->
              <a
                href="https://huggingface.co/{result.model_id}"
                target="_blank"
                rel="noopener noreferrer"
                class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-surface-container-high text-on-surface-variant text-xs font-medium hover:text-primary hover:bg-primary/10 transition-all"
                title="View on HuggingFace"
              >
                <span class="material-symbols-outlined text-sm">open_in_new</span>
                HF Page
              </a>

              <div class="flex-1"></div>

              {#if installed}
                <div class="flex items-center gap-2 text-primary font-bold text-sm">
                  <span
                    class="material-symbols-outlined text-sm"
                    style="font-variation-settings: 'FILL' 1;">check_circle</span
                  >
                  Installed
                </div>
              {:else if downloading}
                <div class="flex items-center gap-2 text-tertiary text-sm font-bold">
                  <span class="material-symbols-outlined text-sm animate-spin"
                    >progress_activity</span
                  >
                  Pulling...
                </div>
              {:else}
                <button
                  class="flex items-center gap-2 bg-tertiary text-on-tertiary px-4 py-2 rounded-lg text-sm font-bold hover:brightness-110 transition-all"
                  onclick={() => showFilePicker(result.model_id)}
                >
                  <span class="material-symbols-outlined text-sm">download</span>
                  Download
                </button>
              {/if}
            </div>

            <!-- File Picker Dropdown -->
            {#if pickerOpen}
              <div class="mt-4 bg-surface-container-high rounded-xl p-3 space-y-1 border border-outline-variant/10">
                {#if filePicker.loading}
                  <div class="flex items-center gap-2 py-3 justify-center text-on-surface-variant text-sm">
                    <span class="material-symbols-outlined text-sm animate-spin">progress_activity</span>
                    Fetching GGUF files...
                  </div>
                {:else if filePicker.files.length === 0}
                  <div class="py-3 text-center text-on-surface-variant text-sm">
                    No GGUF files found in this repo.
                  </div>
                {:else}
                  {@const filteredFiles = filterModelsOnly ? filePicker.files.filter((f) => !isNonModel(f.filename)) : filePicker.files}
                  {@const hiddenCount = filePicker.files.length - filteredFiles.length}
                  <div class="flex items-center justify-between px-2 pb-1">
                    <span class="text-[10px] uppercase font-bold text-on-surface-variant/50 tracking-widest">
                      Select a file to download
                    </span>
                    <label class="flex items-center gap-1.5 cursor-pointer select-none">
                      <input
                        type="checkbox"
                        bind:checked={filterModelsOnly}
                        class="accent-primary w-3.5 h-3.5 rounded cursor-pointer"
                      />
                      <span class="text-[10px] text-on-surface-variant">Models only</span>
                      {#if hiddenCount > 0}
                        <span class="text-[10px] text-on-surface-variant/40">({hiddenCount} hidden)</span>
                      {/if}
                    </label>
                  </div>
                  <div class="max-h-48 overflow-y-auto space-y-1 no-scrollbar">
                    {#each filteredFiles as file}
                      <button
                        class="w-full flex items-center justify-between px-3 py-2 rounded-lg text-left hover:bg-primary/10 transition-colors group/file"
                        onclick={() => pullModel(result.model_id, file.filename, file.size)}
                      >
                        <div class="min-w-0 flex-1 mr-3">
                          <div class="text-sm font-mono text-on-surface truncate">{file.filename}</div>
                        </div>
                        <div class="flex items-center gap-3 flex-shrink-0">
                          <span class="text-xs font-mono text-secondary">{formatSize(file.size)}</span>
                          <span class="material-symbols-outlined text-sm text-tertiary opacity-0 group-hover/file:opacity-100 transition-opacity">download</span>
                        </div>
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

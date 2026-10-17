<script>
  import { onMount } from "svelte";

  let localModels = $state([]);
  let loading = $state(true);

  onMount(async () => {
    await loadLocalModels();
    loading = false;
  });

  async function loadLocalModels() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      localModels = await invoke("list_local_models");
    } catch (e) {
      console.error("Failed to list models:", e);
    }
  }

  let loadingModelId = $state(null);
  let loadError = $state(null);
  let showFilter = $state(false);
  let filterQuery = $state("");
  let backendFilter = $state("all");
  let sortBy = $state("name"); // "name" | "size" | "repo"

  let filteredModels = $derived.by(() => {
    const query = filterQuery.trim().toLowerCase();
    let list = localModels.filter((m) => {
      const matchesQuery =
        !query ||
        (m.id && m.id.toLowerCase().includes(query)) ||
        (m.repo_id && m.repo_id.toLowerCase().includes(query)) ||
        (m.path && m.path.toLowerCase().includes(query));
      const matchesBackend =
        backendFilter === "all" || m.compatible_backends?.includes(backendFilter);
      return matchesQuery && matchesBackend;
    });

    return [...list].sort((a, b) => {
      if (sortBy === "size") {
        return (b.size || 0) - (a.size || 0);
      }
      if (sortBy === "repo") {
        return (a.repo_id || "").localeCompare(b.repo_id || "");
      }
      return (a.id || "").localeCompare(b.id || "");
    });
  });

  function cycleSort() {
    if (sortBy === "name") sortBy = "size";
    else if (sortBy === "size") sortBy = "repo";
    else sortBy = "name";
  }

  async function loadModel(modelId) {
    loadingModelId = modelId;
    loadError = null;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("load_model", { modelId });
    } catch (e) {
      console.error("Load failed:", e);
      loadError = { modelId, title: "Failed to load model", message: String(e) };
    }
    loadingModelId = null;
  }

  function dismissError() {
    loadError = null;
  }

  async function deleteModel(modelId) {
    loadError = null;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      // Unload first if currently loaded in VRAM
      await invoke("unload_model", { modelId }).catch(() => {});
      await invoke("delete_model", { modelId });
      await loadLocalModels();
    } catch (e) {
      console.error("Delete failed:", e);
      loadError = { modelId, title: "Failed to delete model", message: String(e) };
    }
  }

  function formatSize(bytes) {
    if (!bytes || bytes === 0) return "-";
    const units = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return (bytes / Math.pow(1024, i)).toFixed(1) + " " + units[i];
  }

  function estimateVram(bytes) {
    if (!bytes) return "-";
    const gb = bytes / (1024 * 1024 * 1024);
    return `~${(gb * 1.1).toFixed(1)}GB VRAM`;
  }

  function extractFilename(path) {
    if (!path) return "-";
    return path.split(/[\\/]/).pop();
  }
</script>

<div class="min-h-full">
  <!-- Page Content -->
  <div class="max-w-6xl mx-auto p-8">
    <!-- Header -->
    <div class="flex flex-col mb-10">
      <h1 class="text-4xl font-headline font-bold text-on-surface mb-2">Local Library</h1>
      <p class="text-on-surface-variant text-sm max-w-2xl">
        Manage your localized neural assets. Direct GGUF loading and hardware-accelerated
        memory allocation for efficient local inference.
      </p>
    </div>

    <!-- Load / Delete Error Banner -->
    {#if loadError}
      <div class="mb-8 bg-error/5 border border-error/20 rounded-xl p-5">
        <div class="flex items-start gap-4">
          <div class="w-10 h-10 rounded-lg bg-error/10 flex items-center justify-center flex-shrink-0">
            <span class="material-symbols-outlined text-error">error</span>
          </div>
          <div class="flex-1 min-w-0">
            <h3 class="font-headline font-bold text-error text-sm mb-1">{loadError.title || "Failed to load model"}</h3>
            <p class="text-sm text-on-surface-variant leading-relaxed">{loadError.message}</p>
          </div>
          <button
            class="p-2 rounded-lg text-on-surface-variant hover:text-error hover:bg-error/10 transition-colors flex-shrink-0"
            onclick={dismissError}
          >
            <span class="material-symbols-outlined text-sm">close</span>
          </button>
        </div>
      </div>
    {/if}

    {#if loading}
      <div class="flex items-center justify-center h-48">
        <div class="flex items-center gap-3 text-on-surface-variant">
          <span class="material-symbols-outlined animate-spin">progress_activity</span>
          <span class="font-label text-sm uppercase tracking-widest">Scanning local models...</span>
        </div>
      </div>
    {:else}
      <!-- Downloaded Models Section -->
      <section>
        <div class="flex items-center justify-between mb-6">
          <div class="flex items-center gap-3">
            <span class="material-symbols-outlined text-primary">inventory_2</span>
            <h2 class="font-headline text-lg uppercase tracking-widest text-primary">
              Downloaded Models
            </h2>
          </div>
          <div class="flex gap-2">
            <button
              class="bg-surface-container-high px-4 py-2 rounded-lg text-xs font-headline flex items-center gap-2 hover:bg-surface-container-highest transition-colors {showFilter ? 'text-primary border border-primary/30' : 'text-on-surface-variant'}"
              onclick={() => (showFilter = !showFilter)}
            >
              <span class="material-symbols-outlined text-sm">filter_list</span>
              Filter
            </button>
            <button
              class="bg-surface-container-high px-4 py-2 rounded-lg text-xs font-headline flex items-center gap-2 hover:bg-surface-container-highest transition-colors text-on-surface-variant"
              onclick={cycleSort}
              title="Cycle sort order"
            >
              <span class="material-symbols-outlined text-sm">sort</span>
              Sort ({sortBy})
            </button>
          </div>
        </div>

        {#if showFilter}
          <div class="mb-6 p-4 bg-surface-container-lowest rounded-xl flex flex-wrap items-center gap-4 border border-outline-variant/10">
            <input
              type="text"
              placeholder="Filter models by name or repo..."
              bind:value={filterQuery}
              class="flex-1 min-w-[200px] bg-surface-container-low rounded-lg px-3 py-2 text-xs font-mono text-on-surface border border-outline-variant/20 focus:border-primary focus:outline-none"
            />
            <div class="flex items-center gap-1.5">
              {#each ["all", "llamacpp", "candle", "rocm", "cuda"] as b}
                <button
                  class="px-2.5 py-1 rounded text-[10px] font-mono uppercase transition-colors {backendFilter === b ? 'bg-primary text-on-primary font-bold' : 'bg-surface-container-high text-on-surface-variant hover:text-on-surface'}"
                  onclick={() => (backendFilter = b)}
                >
                  {b}
                </button>
              {/each}
            </div>
          </div>
        {/if}

        {#if localModels.length === 0}
          <!-- Empty State -->
          <div
            class="bg-surface-container-lowest rounded-xl p-12 flex flex-col items-center justify-center text-center"
          >
            <div
              class="w-16 h-16 rounded-xl bg-primary/10 flex items-center justify-center mb-6"
            >
              <span class="material-symbols-outlined text-3xl text-primary">download</span>
            </div>
            <h3 class="font-headline text-xl text-on-surface mb-2">No models downloaded</h3>
            <p class="text-on-surface-variant text-sm max-w-md">
              Browse the Model Hub to find and download optimized LLMs for local inference.
              GGUF quantized models are recommended for best performance.
            </p>
          </div>
        {:else if filteredModels.length === 0}
          <div class="bg-surface-container-lowest rounded-xl p-8 text-center text-sm text-on-surface-variant">
            No models match the current filter.
          </div>
        {:else}
          <div class="flex flex-col gap-1">
            <!-- Table Header -->
            <div
              class="grid grid-cols-12 px-6 py-3 text-[10px] font-mono uppercase tracking-widest text-on-surface-variant"
            >
              <div class="col-span-4">Model Identity</div>
              <div class="col-span-3">Backends</div>
              <div class="col-span-2">Requirements</div>
              <div class="col-span-2">Size</div>
              <div class="col-span-1 text-right">Actions</div>
            </div>

            <!-- Model Rows -->
            {#each filteredModels as model}
              <div
                class="grid grid-cols-12 items-center px-6 py-4 bg-surface-container-low hover:bg-surface-container transition-all group rounded-lg mb-2"
              >
                <div class="col-span-4 flex flex-col">
                  <span
                    class="font-headline font-bold text-primary group-hover:text-surface-tint"
                  >
                    {model.repo_id}
                  </span>
                  <span class="text-[10px] font-mono text-on-surface-variant" title={model.path}>
                    {extractFilename(model.path)}
                  </span>
                </div>
                <div class="col-span-3 flex flex-wrap gap-1.5">
                  {#each ['candle', 'llamacpp', 'rocm', 'cuda'] as backend}
                    {#if model.compatible_backends?.includes(backend)}
                      <span
                        class="px-2 py-0.5 rounded text-[10px] font-mono bg-secondary/15 text-secondary"
                      >
                        {backend}
                      </span>
                    {:else}
                      <span
                        class="px-2 py-0.5 rounded text-[10px] font-mono bg-surface-container-highest text-on-surface-variant/40"
                      >
                        {backend}
                      </span>
                    {/if}
                  {/each}
                </div>
                <div class="col-span-2">
                  <span class="text-xs font-mono text-secondary flex items-center gap-1">
                    <span class="material-symbols-outlined text-[14px]">memory</span>
                    {estimateVram(model.size)}
                  </span>
                </div>
                <div class="col-span-2 text-xs font-mono text-on-surface-variant">
                  {formatSize(model.size)}
                </div>
                <div class="col-span-1 flex justify-end gap-3">
                  <button
                    class="text-primary hover:scale-110 transition-transform disabled:opacity-40 disabled:hover:scale-100"
                    onclick={() => loadModel(model.id)}
                    disabled={loadingModelId !== null}
                  >
                    {#if loadingModelId === model.id}
                      <span class="material-symbols-outlined animate-spin">progress_activity</span>
                    {:else}
                      <span
                        class="material-symbols-outlined"
                        style="font-variation-settings: 'FILL' 1;">play_arrow</span
                      >
                    {/if}
                  </button>
                  <button
                    class="text-on-surface-variant hover:text-error transition-colors"
                    onclick={() => deleteModel(model.id)}
                  >
                    <span class="material-symbols-outlined">delete</span>
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  </div>
</div>

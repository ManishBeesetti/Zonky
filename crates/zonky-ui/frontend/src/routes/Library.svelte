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

  async function loadModel(modelId) {
    loadingModelId = modelId;
    loadError = null;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("load_model", { modelId });
    } catch (e) {
      console.error("Load failed:", e);
      loadError = { modelId, message: String(e) };
    }
    loadingModelId = null;
  }

  function dismissError() {
    loadError = null;
  }

  async function deleteModel(modelId) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_model", { modelId });
      await loadLocalModels();
    } catch (e) {
      console.error("Delete failed:", e);
    }
  }

  function formatSize(bytes) {
    if (!bytes || bytes === 0) return "—";
    const units = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return (bytes / Math.pow(1024, i)).toFixed(1) + " " + units[i];
  }

  function estimateVram(bytes) {
    if (!bytes) return "—";
    const gb = bytes / (1024 * 1024 * 1024);
    return `~${(gb * 1.1).toFixed(1)}GB VRAM`;
  }

  function extractFilename(path) {
    if (!path) return "";
    return path.split("/").pop();
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

    <!-- Load Error Banner -->
    {#if loadError}
      <div class="mb-8 bg-error/5 border border-error/20 rounded-xl p-5">
        <div class="flex items-start gap-4">
          <div class="w-10 h-10 rounded-lg bg-error/10 flex items-center justify-center flex-shrink-0">
            <span class="material-symbols-outlined text-error">error</span>
          </div>
          <div class="flex-1 min-w-0">
            <h3 class="font-headline font-bold text-error text-sm mb-1">Failed to load model</h3>
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
              class="bg-surface-container-high px-4 py-2 rounded-lg text-xs font-headline flex items-center gap-2 hover:bg-surface-container-highest transition-colors text-on-surface-variant"
            >
              <span class="material-symbols-outlined text-sm">filter_list</span>
              Filter
            </button>
            <button
              class="bg-surface-container-high px-4 py-2 rounded-lg text-xs font-headline flex items-center gap-2 hover:bg-surface-container-highest transition-colors text-on-surface-variant"
            >
              <span class="material-symbols-outlined text-sm">sort</span>
              Sort
            </button>
          </div>
        </div>

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
            {#each localModels as model}
              <div
                class="grid grid-cols-12 items-center px-6 py-4 bg-surface-container-low hover:bg-surface-container transition-all group rounded-lg mb-2"
              >
                <div class="col-span-4 flex flex-col">
                  <span
                    class="font-headline font-bold text-primary group-hover:text-surface-tint"
                  >
                    {model.repo_id}
                  </span>
                  <span class="text-[10px] font-mono text-on-surface-variant">
                    {model.path}
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

<script>
  import { onMount } from "svelte";

  let config = $state(null);
  let loading = $state(true);

  onMount(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      config = await invoke("get_config");
    } catch (e) {
      console.error("Failed to load config:", e);
    }
    loading = false;
  });

  function getNestedValue(obj, path) {
    return path.split(".").reduce((o, k) => (o ? o[k] : undefined), obj);
  }
</script>

<div class="max-w-4xl mx-auto p-8 space-y-10">
  <!-- Page Header -->
  <div>
    <h1 class="text-4xl font-headline font-bold text-on-surface mb-2">Settings</h1>
    <p class="text-on-surface-variant text-sm">
      Configure inference engine, server, and system preferences.
    </p>
  </div>

  {#if loading}
    <div class="flex items-center justify-center h-48">
      <div class="flex items-center gap-3 text-on-surface-variant">
        <span class="material-symbols-outlined animate-spin">progress_activity</span>
        <span class="font-label text-sm uppercase tracking-widest">Loading configuration...</span>
      </div>
    </div>
  {:else if config}
    <!-- Server Section -->
    <section class="space-y-4">
      <div class="flex items-center gap-3">
        <span class="material-symbols-outlined text-primary">dns</span>
        <h2 class="font-headline text-lg uppercase tracking-widest text-primary">Server</h2>
      </div>
      <div class="bg-surface-container-lowest rounded-xl overflow-hidden">
        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Host</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Server bind address
            </p>
          </div>
          <span class="text-sm font-mono text-on-surface-variant">
            {getNestedValue(config, "server.host") ?? "127.0.0.1"}
          </span>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Port</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              API server port number
            </p>
          </div>
          <span class="text-sm font-mono text-primary">
            {getNestedValue(config, "server.port") ?? "8080"}
          </span>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">CORS Enabled</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Allow cross-origin API requests
            </p>
          </div>
          <div
            class="w-10 h-[22px] rounded-2xl relative cursor-pointer transition-colors {getNestedValue(config, 'server.cors_enabled') ? 'bg-primary' : 'bg-surface-container-highest'}"
          >
            <div
              class="w-[18px] h-[18px] bg-on-surface rounded-full absolute top-[2px] transition-transform {getNestedValue(config, 'server.cors_enabled') ? 'translate-x-[20px]' : 'translate-x-[2px]'}"
            ></div>
          </div>
        </div>
      </div>
    </section>

    <!-- Inference Section -->
    <section class="space-y-4">
      <div class="flex items-center gap-3">
        <span class="material-symbols-outlined text-tertiary">memory</span>
        <h2 class="font-headline text-lg uppercase tracking-widest text-tertiary">Inference</h2>
      </div>
      <div class="bg-surface-container-lowest rounded-xl overflow-hidden">
        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Default Backend</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Primary inference engine
            </p>
          </div>
          <div
            class="px-3 py-1 bg-surface-container-high rounded-lg text-xs font-mono text-on-surface-variant"
          >
            {getNestedValue(config, "default_backend") ?? "auto"}
          </div>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Default Device</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Compute device selection
            </p>
          </div>
          <div
            class="px-3 py-1 bg-surface-container-high rounded-lg text-xs font-mono text-on-surface-variant"
          >
            {getNestedValue(config, "default_device") ?? "auto"}
          </div>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Max Loaded Models</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Concurrent model limit for VRAM management
            </p>
          </div>
          <span class="text-sm font-mono text-primary">
            {getNestedValue(config, "max_loaded_models") ?? "1"}
          </span>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Auto-Evict Models</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              LRU eviction when VRAM is full
            </p>
          </div>
          <div
            class="w-10 h-[22px] rounded-2xl relative cursor-pointer transition-colors {getNestedValue(config, 'auto_evict') ? 'bg-primary' : 'bg-surface-container-highest'}"
          >
            <div
              class="w-[18px] h-[18px] bg-on-surface rounded-full absolute top-[2px] transition-transform {getNestedValue(config, 'auto_evict') ? 'translate-x-[20px]' : 'translate-x-[2px]'}"
            ></div>
          </div>
        </div>
      </div>
    </section>

    <!-- About Section -->
    <section class="space-y-4">
      <div class="flex items-center gap-3">
        <span class="material-symbols-outlined text-secondary">info</span>
        <h2 class="font-headline text-lg uppercase tracking-widest text-secondary">About</h2>
      </div>
      <div class="bg-surface-container-lowest rounded-xl overflow-hidden">
        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Version</p>
          </div>
          <span class="text-sm font-headline font-bold text-primary">0.1.0</span>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Config Path</p>
          </div>
          <span class="text-xs font-mono text-on-surface-variant">~/.config/zonky/config.toml</span>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Cache Path</p>
          </div>
          <span class="text-xs font-mono text-on-surface-variant">
            {config.cache_dir ?? "~/.cache/zonky"}
          </span>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <p class="text-sm font-medium text-on-surface">Engine</p>
          </div>
          <span class="text-xs font-mono text-secondary">Rust + Candle + llama.cpp</span>
        </div>
      </div>
    </section>

    <!-- Danger Zone -->
    <section class="space-y-4 pb-8">
      <div class="flex items-center gap-3">
        <span class="material-symbols-outlined text-error">warning</span>
        <h2 class="font-headline text-lg uppercase tracking-widest text-error">Danger Zone</h2>
      </div>
      <div class="bg-surface-container-lowest rounded-xl p-6 space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium text-on-surface">Clear Model Cache</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Delete all downloaded models from disk
            </p>
          </div>
          <button
            class="bg-error/10 text-error px-4 py-2 rounded-lg text-xs font-bold uppercase tracking-widest hover:bg-error/20 transition-colors"
          >
            Clear Cache
          </button>
        </div>
        <div class="h-[1px] bg-surface-container-high/30"></div>
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-medium text-on-surface">Reset Configuration</p>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Restore all settings to defaults
            </p>
          </div>
          <button
            class="bg-error/10 text-error px-4 py-2 rounded-lg text-xs font-bold uppercase tracking-widest hover:bg-error/20 transition-colors"
          >
            Reset
          </button>
        </div>
      </div>
    </section>
  {:else}
    <div
      class="bg-surface-container-lowest rounded-xl p-12 flex flex-col items-center justify-center text-center"
    >
      <span class="material-symbols-outlined text-3xl text-error mb-4">error</span>
      <h3 class="font-headline text-xl text-on-surface mb-2">Failed to load configuration</h3>
      <p class="text-on-surface-variant text-sm">
        Could not read ~/.config/zonky/config.toml
      </p>
    </div>
  {/if}
</div>

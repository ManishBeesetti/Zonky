<script>
  import { onMount } from "svelte";
  import { refresh as refreshHardware } from "../stores/hardware.svelte.js";

  let config = $state(null);
  let loading = $state(true);
  let saving = $state(false);
  let statusMessage = $state("");
  let statusTone = $state("success");

  onMount(async () => {
    await loadConfig();
  });

  async function loadConfig() {
    loading = true;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const raw = await invoke("get_config");
      config = normalizeConfig(raw);
    } catch (e) {
      console.error("Failed to load config:", e);
    }
    loading = false;
  }

  function normalizeConfig(raw) {
    if (!raw) return null;
    return {
      default_backend: raw.default_backend ?? "auto",
      default_device: typeof raw.default_device === "string" ? raw.default_device : "auto",
      cache_dir: raw.cache_dir ?? "~/.cache/zonky",
      config_path: raw.config_path ?? "~/.config/zonky/config.toml",
      auto_evict: raw.auto_evict ?? true,
      max_loaded_models: Number(raw.max_loaded_models ?? 4),
      server: {
        host: raw.server?.host ?? "127.0.0.1",
        port: Number(raw.server?.port ?? 8080),
        api_key: raw.server?.api_key ?? "",
        cors_enabled: raw.server?.cors_enabled ?? true,
      },
    };
  }

  async function saveSettings() {
    if (!config) return;
    if (!config.server.host || !config.server.host.trim()) {
      statusMessage = "Server host cannot be empty.";
      statusTone = "error";
      return;
    }
    const port = Number(config.server.port);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      statusMessage = "Server port must be between 1 and 65535.";
      statusTone = "error";
      return;
    }
    const maxModels = Number(config.max_loaded_models);
    if (!Number.isInteger(maxModels) || maxModels < 1) {
      statusMessage = "Max loaded models must be at least 1.";
      statusTone = "error";
      return;
    }

    saving = true;
    statusMessage = "";
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const payload = {
        ...config,
        max_loaded_models: maxModels,
        server: {
          ...config.server,
          host: config.server.host.trim(),
          port,
          api_key: config.server.api_key?.trim() ? config.server.api_key.trim() : null,
        },
      };
      const updated = await invoke("save_config", { config: payload });
      if (updated) {
        config = normalizeConfig(updated);
      }
      await refreshHardware();
      statusMessage = "Settings saved successfully.";
      statusTone = "success";
    } catch (e) {
      statusMessage = `Failed to save settings: ${e}`;
      statusTone = "error";
    }
    saving = false;
  }

  async function handleResetConfig() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const reset = await invoke("reset_config");
      if (reset) {
        config = normalizeConfig(reset);
      } else {
        config = normalizeConfig({
          default_backend: "auto",
          default_device: "auto",
          cache_dir: "~/.cache/zonky",
          auto_evict: true,
          max_loaded_models: 4,
          server: { host: "127.0.0.1", port: 8080, api_key: null, cors_enabled: true },
        });
      }
      await refreshHardware();
      statusMessage = "Configuration reset to defaults.";
      statusTone = "success";
    } catch (e) {
      statusMessage = `Failed to reset configuration: ${e}`;
      statusTone = "error";
    }
  }

  async function handleClearCache() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const removed = await invoke("clear_model_cache");
      await refreshHardware();
      statusMessage = `Cleared model cache (${removed ?? 0} models removed).`;
      statusTone = "success";
    } catch (e) {
      statusMessage = `Failed to clear cache: ${e}`;
      statusTone = "error";
    }
  }
</script>

<div class="max-w-4xl mx-auto p-8 space-y-10">
  <!-- Page Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-4xl font-headline font-bold text-on-surface mb-2">Settings</h1>
      <p class="text-on-surface-variant text-sm">
        Configure inference engine, server, and system preferences.
      </p>
    </div>
    {#if config}
      <button
        class="bg-primary text-on-primary px-5 py-2.5 rounded-xl font-headline font-bold text-xs uppercase tracking-widest hover:shadow-[0_0_15px_rgba(76,214,255,0.4)] transition-all disabled:opacity-50"
        onclick={saveSettings}
        disabled={saving}
      >
        {saving ? "Saving..." : "Save Configuration"}
      </button>
    {/if}
  </div>

  {#if statusMessage}
    <div
      class="p-4 rounded-xl border flex items-center justify-between {statusTone === 'error'
        ? 'bg-error/10 border-error/30 text-error'
        : 'bg-secondary/10 border-secondary/30 text-secondary'}"
    >
      <span class="text-sm font-mono">{statusMessage}</span>
      <button
        class="text-xs uppercase font-bold opacity-70 hover:opacity-100"
        onclick={() => (statusMessage = "")}
      >
        Dismiss
      </button>
    </div>
  {/if}

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
            <label for="server-host-input" class="text-sm font-medium text-on-surface block">Host</label>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Server bind address
            </p>
          </div>
          <input
            id="server-host-input"
            type="text"
            bind:value={config.server.host}
            class="bg-surface-container-high text-on-surface text-sm font-mono rounded-lg px-3 py-1.5 w-44 text-right border border-outline-variant/20 focus:border-primary focus:outline-none"
          />
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <label for="server-port-input" class="text-sm font-medium text-on-surface block">Port</label>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              API server port number
            </p>
          </div>
          <input
            id="server-port-input"
            type="number"
            min="1"
            max="65535"
            bind:value={config.server.port}
            class="bg-surface-container-high text-primary text-sm font-mono rounded-lg px-3 py-1.5 w-28 text-right border border-outline-variant/20 focus:border-primary focus:outline-none"
          />
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <label for="server-apikey-input" class="text-sm font-medium text-on-surface block">API Key (Optional)</label>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Bearer token required for /v1/* and /api/* requests
            </p>
          </div>
          <input
            id="server-apikey-input"
            type="password"
            placeholder="None (public local API)"
            bind:value={config.server.api_key}
            class="bg-surface-container-high text-on-surface text-sm font-mono rounded-lg px-3 py-1.5 w-56 text-right border border-outline-variant/20 focus:border-primary focus:outline-none"
          />
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
          <button
            type="button"
            aria-label="Toggle CORS Enabled"
            onclick={() => (config.server.cors_enabled = !config.server.cors_enabled)}
            class="w-10 h-[22px] rounded-2xl relative cursor-pointer transition-colors {config.server.cors_enabled ? 'bg-primary' : 'bg-surface-container-highest'}"
          >
            <div
              class="w-[18px] h-[18px] bg-on-surface rounded-full absolute top-[2px] transition-transform {config.server.cors_enabled ? 'translate-x-[20px]' : 'translate-x-[2px]'}"
            ></div>
          </button>
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
          <div class="flex gap-1 bg-surface-container-high p-1 rounded-lg">
            {#each ["auto", "candle", "llamacpp"] as backendOpt}
              <button
                type="button"
                onclick={() => (config.default_backend = backendOpt)}
                class="px-2.5 py-1 rounded text-xs font-mono transition-colors {config.default_backend === backendOpt ? 'bg-primary text-on-primary font-bold' : 'text-on-surface-variant hover:text-on-surface'}"
              >
                {backendOpt}
              </button>
            {/each}
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
          <div class="flex gap-1 bg-surface-container-high p-1 rounded-lg">
            {#each ["auto", "cpu", "metal"] as deviceOpt}
              <button
                type="button"
                onclick={() => (config.default_device = deviceOpt)}
                class="px-2.5 py-1 rounded text-xs font-mono transition-colors {config.default_device === deviceOpt ? 'bg-primary text-on-primary font-bold' : 'text-on-surface-variant hover:text-on-surface'}"
              >
                {deviceOpt}
              </button>
            {/each}
          </div>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <label for="max-models-input" class="text-sm font-medium text-on-surface block">Max Loaded Models</label>
            <p class="text-[10px] text-on-surface-variant uppercase tracking-widest mt-0.5">
              Concurrent model limit for VRAM management
            </p>
          </div>
          <input
            id="max-models-input"
            type="number"
            min="1"
            max="16"
            bind:value={config.max_loaded_models}
            class="bg-surface-container-high text-primary text-sm font-mono rounded-lg px-3 py-1.5 w-20 text-right border border-outline-variant/20 focus:border-primary focus:outline-none"
          />
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
          <button
            type="button"
            aria-label="Toggle Auto-Evict Models"
            onclick={() => (config.auto_evict = !config.auto_evict)}
            class="w-10 h-[22px] rounded-2xl relative cursor-pointer transition-colors {config.auto_evict ? 'bg-primary' : 'bg-surface-container-highest'}"
          >
            <div
              class="w-[18px] h-[18px] bg-on-surface rounded-full absolute top-[2px] transition-transform {config.auto_evict ? 'translate-x-[20px]' : 'translate-x-[2px]'}"
            ></div>
          </button>
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
          <span class="text-xs font-mono text-on-surface-variant">{config.config_path}</span>
        </div>

        <div class="h-[1px] bg-surface-container-high/30 mx-6"></div>

        <div
          class="flex items-center justify-between px-6 py-4 hover:bg-surface-container-low/50 transition-colors"
        >
          <div>
            <label for="cache-dir-input" class="text-sm font-medium text-on-surface block">Cache Path</label>
          </div>
          <input
            id="cache-dir-input"
            type="text"
            bind:value={config.cache_dir}
            class="bg-surface-container-high text-on-surface-variant text-xs font-mono rounded-lg px-3 py-1.5 w-72 text-right border border-outline-variant/20 focus:border-primary focus:outline-none"
          />
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
            onclick={handleClearCache}
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
            onclick={handleResetConfig}
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


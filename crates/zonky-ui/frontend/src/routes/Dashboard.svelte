<script>
  import { onMount } from "svelte";
  import { getHardware, refresh } from "../stores/hardware.svelte.js";

  let hw = $derived(getHardware());
  let setupStatus = $state(null);
  let serverStatus = $state({ running: false, host: "127.0.0.1", port: 8080, url: "http://127.0.0.1:8080" });
  let serverToggling = $state(false);
  let logs = $state([]);
  let copied = $state(false);

  const gaugeRadius = 70;
  const gaugeCircumference = 2 * Math.PI * gaugeRadius;

  onMount(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const [setup, status] = await Promise.all([
        invoke("get_setup_status"),
        invoke("get_server_status"),
      ]);
      setupStatus = setup;
      serverStatus = status;
    } catch (_) {}
  });

  async function toggleServer() {
    serverToggling = true;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      if (serverStatus.running) {
        await invoke("stop_server");
        serverStatus = { ...serverStatus, running: false };
        addLog("INFO", "Server stopped");
      } else {
        const result = await invoke("start_server");
        serverStatus = result;
        addLog("INFO", `Server started at ${result.url}`);
      }
    } catch (e) {
      addLog("ERROR", `Server toggle failed: ${e}`);
    }
    serverToggling = false;
  }

  async function ejectModel(modelId) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("unload_model", { modelId });
      addLog("INFO", `Model '${modelId}' ejected from VRAM`);
      await refresh();
    } catch (e) {
      addLog("ERROR", `Eject failed: ${e}`);
    }
  }

  function addLog(level, message) {
    const ts = new Date().toLocaleTimeString("en-US", { hour12: false });
    logs = [{ ts, level, message }, ...logs].slice(0, 100);
  }

  function copyUrl() {
    navigator.clipboard.writeText(serverStatus.url);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function formatGB(bytes) {
    if (!bytes) return "0";
    return (bytes / (1024 * 1024 * 1024)).toFixed(1);
  }

  function formatSize(bytes) {
    if (!bytes) return "-";
    const gb = bytes / (1024 * 1024 * 1024);
    if (gb >= 1) return gb.toFixed(2) + " GB";
    return (bytes / (1024 * 1024)).toFixed(0) + " MB";
  }

  function gaugeOffset(percent) {
    return gaugeCircumference - (percent / 100) * gaugeCircumference;
  }
</script>

<div class="p-8 max-w-7xl mx-auto space-y-6">
  <!-- Server Header Bar -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="font-headline text-3xl font-bold tracking-tight text-on-surface">Compute Dashboard</h1>
      <p class="text-on-surface-variant text-sm mt-1">Server control & hardware orchestration</p>
    </div>
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-3 px-4 py-2 bg-surface-container-lowest rounded-xl">
        <span class="text-[10px] font-label uppercase tracking-widest font-bold text-on-surface-variant">Status:</span>
        <button
          class="relative w-12 h-[26px] rounded-2xl transition-colors cursor-pointer {serverStatus.running ? 'bg-secondary' : 'bg-surface-container-highest'}"
          onclick={toggleServer}
          disabled={serverToggling}
        >
          <div class="w-[22px] h-[22px] bg-on-surface rounded-full absolute top-[2px] transition-transform {serverStatus.running ? 'translate-x-[26px]' : 'translate-x-[2px]'}"></div>
        </button>
        <span class="text-sm font-bold {serverStatus.running ? 'text-secondary' : 'text-on-surface-variant'}">
          {serverStatus.running ? "Running" : "Stopped"}
        </span>
      </div>
      {#if serverStatus.running}
        <div class="flex items-center gap-2 px-4 py-2 bg-surface-container-lowest rounded-xl">
          <span class="text-[10px] font-label uppercase tracking-widest font-bold text-on-surface-variant">Reachable at:</span>
          <span class="text-sm font-mono text-primary">{serverStatus.url}</span>
          <button class="p-1 rounded hover:bg-primary/10 transition-colors" onclick={copyUrl} title="Copy URL">
            <span class="material-symbols-outlined text-sm text-primary">{copied ? "check" : "content_copy"}</span>
          </button>
        </div>
      {/if}
    </div>
  </div>

  <!-- Loaded Models (LM Studio style) -->
  <section class="space-y-3">
    {#if hw.loadedModels.length > 0}
      {#each hw.loadedModels as model}
        <div class="bg-surface-container-lowest rounded-xl p-5 flex items-center gap-6 border-l-2 border-secondary">
          <div class="px-3 py-1 bg-secondary/15 rounded-lg">
            <span class="text-[10px] font-headline font-bold uppercase tracking-widest text-secondary">Ready</span>
          </div>
          <div class="flex items-center gap-2 flex-1 min-w-0">
            <span class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold">llm</span>
            <span class="px-3 py-1 bg-surface-container-high rounded-lg text-sm font-mono text-primary truncate">{model.id}</span>
          </div>
          <span class="text-xs font-mono text-on-surface-variant flex-shrink-0">Size {formatSize(model.vram_usage)}</span>
          <button
            class="flex items-center gap-2 px-4 py-2 rounded-lg bg-surface-container-high text-on-surface-variant text-xs font-bold uppercase tracking-widest hover:text-error hover:bg-error/10 transition-colors flex-shrink-0"
            onclick={() => ejectModel(model.id)}
          >
            <span class="material-symbols-outlined text-sm">eject</span> Eject
          </button>
        </div>
      {/each}
    {:else}
      <div class="bg-surface-container-lowest rounded-xl p-8 flex flex-col items-center justify-center text-center">
        <span class="material-symbols-outlined text-3xl text-on-surface-variant/30 mb-3">smart_toy</span>
        <p class="text-sm text-on-surface-variant">No models loaded. Go to Library to load a model into VRAM.</p>
      </div>
    {/if}
  </section>

  <!-- Bento Grid -->
  <div class="grid grid-cols-12 gap-6">
    <!-- GPU Monitor (8 cols) -->
    <div class="col-span-12 lg:col-span-8 bg-surface-container-lowest rounded-xl p-6 border-l-2 border-secondary">
      <div class="flex justify-between items-start mb-6">
        <div>
          <span class="text-on-surface-variant font-label text-[10px] uppercase tracking-[0.2em]">Hardware Monitor</span>
          <h2 class="font-headline text-xl text-on-surface mt-1">{hw.gpuName}</h2>
        </div>
        <span class="font-headline text-4xl text-secondary">{hw.vramPercent}<span class="text-sm ml-1">%</span></span>
      </div>
      <div class="flex gap-12 items-center">
        <div class="relative w-40 h-40 flex-shrink-0">
          <svg class="w-full h-full transform -rotate-90">
            <circle class="text-surface-container-high" cx="80" cy="80" r={gaugeRadius} fill="transparent" stroke="currentColor" stroke-width="4" />
            <circle class="text-secondary transition-all duration-1000" cx="80" cy="80" r={gaugeRadius} fill="transparent" stroke="currentColor" stroke-dasharray={gaugeCircumference} stroke-dashoffset={gaugeOffset(hw.vramPercent)} stroke-width="8" stroke-linecap="round" />
          </svg>
          <div class="absolute inset-0 flex flex-col items-center justify-center">
            <span class="font-headline text-xl text-on-surface">VRAM</span>
            <span class="text-[10px] text-on-surface-variant">{hw.vramPercent < 50 ? "LOW" : hw.vramPercent < 80 ? "OPTIMAL" : "HIGH"}</span>
          </div>
        </div>
        <div class="flex-1 space-y-4">
          <div>
            <div class="flex justify-between mb-2">
              <span class="font-headline text-3xl text-primary">{formatGB(hw.vramUsed)} <span class="text-sm">GB</span></span>
              <span class="text-on-surface-variant text-xs self-end">/ {formatGB(hw.vramTotal)} GB Total</span>
            </div>
            <div class="h-2 w-full bg-surface-container-high rounded-2xl overflow-hidden">
              <div class="h-full bg-primary rounded-2xl transition-all duration-700" style="width: {hw.vramPercent}%"></div>
            </div>
          </div>
          <div class="grid grid-cols-2 gap-4">
            <div class="bg-surface-container-low rounded-lg p-3">
              <p class="text-[10px] uppercase tracking-widest text-on-surface-variant font-bold mb-1">System RAM</p>
              <p class="text-sm font-mono text-on-surface">{formatGB(hw.availableMemory)} / {formatGB(hw.totalMemory)} GB</p>
            </div>
            <div class="bg-surface-container-low rounded-lg p-3">
              <p class="text-[10px] uppercase tracking-widest text-on-surface-variant font-bold mb-1">GPU Vendor</p>
              <p class="text-sm font-mono text-tertiary">{hw.gpuVendor || "-"}</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Stats (4 cols) -->
    <div class="col-span-12 lg:col-span-4 bg-surface-container-lowest rounded-xl p-6 flex flex-col justify-between">
      <div>
        <span class="text-on-surface-variant font-label text-[10px] uppercase tracking-[0.2em]">Inference Stats</span>
        <h2 class="font-headline text-xl text-on-surface mt-1">Performance</h2>
      </div>
      <div class="space-y-4 py-4">
        <div class="bg-surface-container-low rounded-lg p-4 border-l-2 border-primary">
          <p class="text-[10px] uppercase tracking-widest text-on-surface-variant font-bold">Throughput</p>
          <p class="font-headline text-3xl text-primary font-bold">-</p>
          <p class="text-[10px] uppercase tracking-widest text-on-surface-variant">tok/s</p>
        </div>
        <div class="bg-surface-container-low rounded-lg p-4 border-l-2 border-tertiary">
          <p class="text-[10px] uppercase tracking-widest text-on-surface-variant font-bold">Latency</p>
          <p class="font-headline text-3xl text-tertiary font-bold">-</p>
          <p class="text-[10px] uppercase tracking-widest text-on-surface-variant">ms (first token)</p>
        </div>
        <div class="bg-surface-container-low rounded-lg p-4 border-l-2 border-secondary">
          <p class="text-[10px] uppercase tracking-widest text-on-surface-variant font-bold">Models in VRAM</p>
          <p class="font-headline text-3xl text-secondary font-bold">{hw.loadedModels.length}</p>
        </div>
      </div>
    </div>

    <!-- Endpoints -->
    {#if serverStatus.running}
      <div class="col-span-12 bg-surface-container-lowest rounded-xl overflow-hidden">
        <div class="px-6 py-4 flex justify-between items-center" style="background: rgba(28, 27, 28, 0.3);">
          <h3 class="font-headline text-md text-on-surface">Supported Endpoints (OpenAI-like)</h3>
          <span class="text-[10px] font-bold text-secondary px-2 py-1 bg-secondary/10 rounded uppercase tracking-widest">Active</span>
        </div>
        <div class="p-6 grid grid-cols-1 md:grid-cols-2 gap-3">
          {#each [
            { method: "POST", path: "/v1/chat/completions", desc: "Chat completions (streaming + batch)" },
            { method: "POST", path: "/v1/completions", desc: "Text completions" },
            { method: "GET", path: "/v1/models", desc: "List available models" },
            { method: "GET", path: "/health", desc: "Server health check" },
          ] as endpoint}
            <div class="flex items-center gap-3 px-4 py-3 bg-surface-container-low rounded-lg">
              <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold {endpoint.method === 'POST' ? 'bg-tertiary/15 text-tertiary' : 'bg-secondary/15 text-secondary'}">{endpoint.method}</span>
              <span class="text-sm font-mono text-primary">{endpoint.path}</span>
              <span class="text-xs text-on-surface-variant ml-auto">{endpoint.desc}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Dependencies -->
    {#if setupStatus?.deps?.length > 0}
      <div class="col-span-12 bg-surface-container-lowest rounded-xl overflow-hidden">
        <div class="px-6 py-4 flex justify-between items-center" style="background: rgba(28, 27, 28, 0.5);">
          <h3 class="font-headline text-md text-on-surface">Compute Dependencies</h3>
          <span class="text-[10px] font-bold px-2 py-1 rounded uppercase tracking-widest {setupStatus.compute_ready ? 'text-secondary bg-secondary/10' : 'text-error bg-error/10'}">
            {setupStatus.compute_ready ? "All Installed" : "Missing Deps"}
          </span>
        </div>
        <table class="w-full text-left border-collapse">
          <thead class="text-[10px] uppercase tracking-widest text-on-surface-variant" style="background: rgba(28, 27, 28, 0.3);">
            <tr><th class="px-6 py-3 font-bold">Package</th><th class="px-6 py-3 font-bold">Description</th><th class="px-6 py-3 font-bold">Status</th></tr>
          </thead>
          <tbody class="text-sm">
            {#each setupStatus.deps as dep}
              <tr class="hover:bg-surface-container-high/50 transition-colors">
                <td class="px-6 py-4 font-mono text-primary text-xs">{dep.name}</td>
                <td class="px-6 py-4 text-on-surface-variant">{dep.description}</td>
                <td class="px-6 py-4">
                  {#if dep.installed}
                    <span class="text-secondary flex items-center gap-1"><span class="material-symbols-outlined text-sm" style="font-variation-settings: 'FILL' 1;">check_circle</span> Installed</span>
                  {:else}
                    <span class="text-error flex items-center gap-1"><span class="material-symbols-outlined text-sm">cancel</span> Missing</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    <!-- Dev Logs -->
    <div class="col-span-12 bg-surface-container-lowest rounded-xl overflow-hidden">
      <div class="px-6 py-4 flex justify-between items-center" style="background: rgba(28, 27, 28, 0.3);">
        <h3 class="font-headline text-md text-on-surface">Developer Logs</h3>
        <button class="p-1.5 rounded-lg hover:bg-surface-container-highest transition-colors text-on-surface-variant" onclick={() => (logs = [])} title="Clear logs">
          <span class="material-symbols-outlined text-sm">delete</span>
        </button>
      </div>
      <div class="h-48 overflow-y-auto no-scrollbar p-4 font-mono text-xs space-y-1">
        {#if logs.length === 0}
          <p class="text-on-surface-variant/40 text-center py-8">No log entries yet. Start the server or load a model to see activity.</p>
        {:else}
          {#each logs as log}
            <div class="flex gap-3">
              <span class="text-on-surface-variant/50 flex-shrink-0">{log.ts}</span>
              <span class="flex-shrink-0 {log.level === 'ERROR' ? 'text-error' : log.level === 'WARN' ? 'text-tertiary' : 'text-secondary'}">[{log.level}]</span>
              <span class="text-on-surface-variant">{log.message}</span>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
</div>

<script>
  import { onMount, onDestroy } from "svelte";
  import Dashboard from "./routes/Dashboard.svelte";
  import Library from "./routes/Library.svelte";
  import Models from "./routes/Models.svelte";
  import Downloads from "./routes/Downloads.svelte";
  import Chat from "./routes/Chat.svelte";
  import Settings from "./routes/Settings.svelte";
  import { activeCount } from "./stores/downloads.svelte.js";
  import { getHardware, startPolling, stopPolling } from "./stores/hardware.svelte.js";

  let currentPage = $state("dashboard");
  let systemStatus = $state("ready");
  let serverRunning = $state(false);
  let dlCount = $derived(activeCount());
  let hw = $derived(getHardware());
  let activeModel = $derived(
    hw.loadedModels.length > 0 ? hw.loadedModels.map((m) => m.id).join(", ") : null
  );

  const navItems = [
    { id: "dashboard", icon: "dashboard", label: "Dashboard" },
    { id: "library", icon: "library_books", label: "Library" },
    { id: "models", icon: "extension", label: "Models" },
    { id: "downloads", icon: "cloud_download", label: "Downloads" },
    { id: "chat", icon: "chat_bubble", label: "Chat" },
  ];

  onMount(() => {
    startPolling(3000);
  });

  onDestroy(() => {
    stopPolling();
  });

  function formatGB(bytes) {
    if (!bytes) return "0";
    return (bytes / (1024 * 1024 * 1024)).toFixed(1);
  }

  function statusLabel() {
    if (systemStatus === "ready") return "System Ready";
    if (systemStatus === "loading") return "Loading...";
    return "Offline";
  }

  function statusColor() {
    if (systemStatus === "ready") return "bg-secondary";
    if (systemStatus === "loading") return "bg-tertiary";
    return "bg-error";
  }
</script>

<!-- Navigation Drawer -->
<nav class="fixed left-0 top-0 h-screen w-20 flex flex-col items-center py-6 bg-surface-container-lowest z-50">
  <div class="mb-10">
    <span class="text-primary font-bold tracking-widest font-headline text-xs">ZONKY</span>
  </div>

  <div class="flex flex-col gap-4 flex-1">
    {#each navItems as item}
      <button
        class="p-3 flex flex-col items-center gap-1 rounded-lg transition-all duration-100 {currentPage === item.id
          ? 'text-primary bg-surface-container-highest scale-95'
          : 'text-on-surface-variant opacity-60 hover:bg-surface-container-highest hover:opacity-100'}"
        onclick={() => (currentPage = item.id)}
      >
        <span
          class="material-symbols-outlined"
          style={currentPage === item.id ? "font-variation-settings: 'FILL' 1;" : ""}
          >{item.icon}</span
        >
        <span class="text-[10px] font-headline tracking-tight">{item.label}</span>
      </button>
    {/each}
  </div>

  <button
    class="p-3 flex flex-col items-center gap-1 rounded-lg transition-all duration-100 {currentPage === 'settings'
      ? 'text-primary bg-surface-container-highest scale-95'
      : 'text-on-surface-variant opacity-60 hover:bg-surface-container-highest hover:opacity-100'}"
    onclick={() => (currentPage = "settings")}
  >
    <span
      class="material-symbols-outlined"
      style={currentPage === "settings" ? "font-variation-settings: 'FILL' 1;" : ""}
      >settings</span
    >
    <span class="text-[10px] font-headline tracking-tight">Settings</span>
  </button>
</nav>

<!-- Top App Bar -->
<header
  class="fixed top-0 right-0 left-20 z-40 flex items-center justify-between px-6 h-14 bg-surface/80 backdrop-blur-xl"
>
  <div class="flex items-center gap-3">
    <span class="material-symbols-outlined text-primary">search</span>
    <span class="text-sm text-on-surface-variant">System Search</span>
  </div>

  <div class="flex items-center gap-6">
    {#if activeModel}
      <div
        class="flex items-center gap-2 px-3 py-1 bg-surface-container-low rounded-2xl"
      >
        <span class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant"
          >Active Model:</span
        >
        <span class="text-sm font-medium text-primary">{activeModel}</span>
      </div>
    {/if}
    <div class="flex items-center gap-2 px-3 py-1 bg-surface-container-low rounded-2xl">
      <span class="w-2 h-2 rounded-full {statusColor()}"></span>
      <span class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant"
        >{statusLabel()}</span
      >
    </div>
    <button
      class="relative text-on-surface-variant hover:text-primary transition-colors {currentPage === 'downloads' ? 'text-primary' : ''}"
      onclick={() => (currentPage = "downloads")}
      title="Downloads"
    >
      <span class="material-symbols-outlined"
        style={dlCount > 0 ? "font-variation-settings: 'FILL' 1;" : ""}
      >{dlCount > 0 ? "downloading" : "cloud_download"}</span>
      {#if dlCount > 0}
        <span class="absolute -top-1 -right-1 w-4 h-4 bg-tertiary text-on-tertiary text-[9px] font-bold rounded-full flex items-center justify-center animate-pulse">
          {dlCount}
        </span>
      {/if}
    </button>
    <button class="text-on-surface-variant hover:text-primary transition-colors">
      <span class="material-symbols-outlined">more_vert</span>
    </button>
  </div>

  <!-- Bottom separator via background shift -->
  <div class="absolute bottom-0 left-0 right-0 h-[1px] bg-surface-container-low"></div>
</header>

<!-- Main Content — all pages stay mounted, hidden via CSS to preserve state -->
<main class="ml-20 mt-14 mb-8 h-[calc(100vh-56px-32px)] overflow-y-auto no-scrollbar bg-background">
  <div class="h-full" hidden={currentPage !== "dashboard"}><Dashboard /></div>
  <div class="h-full" hidden={currentPage !== "library"}><Library /></div>
  <div class="h-full" hidden={currentPage !== "models"}><Models /></div>
  <div class="h-full" hidden={currentPage !== "downloads"}><Downloads /></div>
  <div class="h-full" hidden={currentPage !== "chat"}><Chat /></div>
  <div class="h-full" hidden={currentPage !== "settings"}><Settings /></div>
</main>

<!-- Footer Status Bar -->
<footer
  class="fixed bottom-0 right-0 left-20 z-50 flex items-center justify-between px-4 bg-surface-container-lowest h-8 font-label text-[10px] uppercase tracking-widest"
>
  <div class="flex items-center gap-4">
    <span class="text-secondary font-bold">{statusLabel()}</span>
    {#if activeModel}
      <span class="text-on-surface-variant opacity-40">|</span>
      <div class="flex items-center gap-2">
        <span class="w-1.5 h-1.5 rounded-full bg-secondary animate-pulse"></span>
        <span class="text-on-surface-variant">{activeModel}</span>
      </div>
    {/if}
  </div>
  <div class="flex items-center gap-6">
    {#if serverRunning}
      <span class="text-on-surface-variant">Server: Localhost:8080</span>
    {/if}
    {#if hw.vramTotal > 0}
      <div class="flex items-center gap-1">
        <span class="material-symbols-outlined text-[12px]">memory</span>
        <span class="text-secondary">VRAM: {formatGB(hw.vramUsed)}GB / {formatGB(hw.vramTotal)}GB</span>
      </div>
    {/if}
  </div>

  <div class="absolute top-0 left-0 right-0 h-[1px] bg-surface-container-low"></div>
</footer>

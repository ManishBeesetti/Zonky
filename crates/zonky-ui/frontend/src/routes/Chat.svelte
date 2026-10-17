<script>
  import { onMount } from "svelte";
  import { getHardware } from "../stores/hardware.svelte.js";

  let messages = $state([]);
  let input = $state("");
  let selectedModel = $state("");
  let sending = $state(false);
  let temperature = $state(0.75);
  let topP = $state(0.9);
  let repeatPenalty = $state(1.1);
  let showConfig = $state(true);
  let loadingModels = $state(true);

  let hw = $derived(getHardware());
  let loadedModels = $derived(hw.loadedModels);

  // Auto-select first model when models become available
  $effect(() => {
    if (loadedModels.length > 0 && !selectedModel) {
      selectedModel = loadedModels[0].id;
    }
    loadingModels = false;
  });

  function fetchLoadedModels() {
    // Now handled by hardware store polling - this is just for the Refresh button
    loadingModels = false;
  }

  function formatGB(bytes) {
    if (!bytes) return "-";
    return (bytes / (1024 * 1024 * 1024)).toFixed(1);
  }

  /** Parse assistant content into renderable blocks: text paragraphs + code blocks */
  function parseContent(text) {
    const blocks = [];
    const codeRegex = /```(\w*)?\n([\s\S]*?)```/g;
    let lastIdx = 0;
    let match;
    while ((match = codeRegex.exec(text)) !== null) {
      // Text before code block
      const before = text.slice(lastIdx, match.index).trim();
      if (before) blocks.push({ type: "text", content: before });
      blocks.push({ type: "code", lang: match[1] || "text", content: match[2].trimEnd() });
      lastIdx = match.index + match[0].length;
    }
    const after = text.slice(lastIdx).trim();
    if (after) blocks.push({ type: "text", content: after });
    if (blocks.length === 0) blocks.push({ type: "text", content: text });
    return blocks;
  }

  function copyCode(code) {
    navigator.clipboard.writeText(code);
  }

  async function sendMessage() {
    const text = input.trim();
    if (!text || sending || !selectedModel) return;

    messages = [...messages, { role: "user", content: text }];
    input = "";
    sending = true;

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const response = await invoke("chat_complete", {
        model: selectedModel,
        messages: messages.map((m) => ({ role: m.role, content: m.content })),
      });
      messages = [...messages, { role: "assistant", content: response.content }];
    } catch (e) {
      messages = [...messages, { role: "assistant", content: `Error: ${e}` }];
    }
    sending = false;
  }

  function handleKey(e) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }

  function clearChat() {
    messages = [];
  }
</script>

<div class="flex h-full overflow-hidden">
  <!-- Chat Main Area -->
  <section class="flex-1 flex flex-col relative h-full">
    <!-- Model Picker Bar -->
    <div class="px-6 py-3 border-b border-outline-variant/10 flex items-center gap-4 bg-surface-container-lowest/50 backdrop-blur-sm">
      <span class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold">Model</span>
      {#if loadedModels.length > 0}
        <select
          bind:value={selectedModel}
          class="bg-surface-container-low text-on-surface text-sm rounded-lg px-3 py-1.5 border border-outline-variant/20 focus:border-primary focus:outline-none font-mono appearance-none cursor-pointer min-w-[200px]"
        >
          {#each loadedModels as m}
            <option value={m.id}>{m.id}</option>
          {/each}
        </select>
      {:else}
        <span class="text-sm text-on-surface-variant/60 italic">No models loaded</span>
      {/if}
      <button
        class="ml-auto flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-container-highest text-on-surface-variant hover:text-on-surface transition-colors text-[10px] font-label uppercase tracking-widest font-bold"
        onclick={fetchLoadedModels}
      >
        <span class="material-symbols-outlined text-sm">refresh</span>
        Refresh
      </button>
    </div>

    <!-- Messages Stream -->
    <div class="flex-1 overflow-y-auto px-6 py-8 space-y-12 no-scrollbar">
      {#if messages.length === 0}
        <!-- Empty State -->
        <div class="flex flex-col items-center justify-center h-full text-center">
          <div
            class="w-16 h-16 rounded-xl bg-primary/10 flex items-center justify-center mb-6 border border-primary/20"
          >
            <span class="material-symbols-outlined text-3xl text-primary">smart_toy</span>
          </div>
          <h2 class="font-headline text-2xl text-on-surface mb-2">Start Inference</h2>
          <p class="text-on-surface-variant text-sm max-w-md">
            Load a model from the Library, then begin a conversation. Zonky will run inference
            locally on your hardware.
          </p>
          <div class="mt-8 p-4 bg-primary/5 rounded-xl border border-primary/10 max-w-md">
            <div class="flex gap-3">
              <span class="material-symbols-outlined text-primary text-sm">info</span>
              <p class="text-[10px] text-primary/80 leading-relaxed uppercase tracking-wider font-medium">
                {#if loadedModels.length === 0}
                  No model currently loaded. Go to Library to load a model first.
                {:else}
                  Select a model above and start chatting.
                {/if}
              </p>
            </div>
          </div>
        </div>
      {:else}
        <!-- System Configuration Message -->
        <div class="max-w-3xl mx-auto flex gap-6">
          <div
            class="w-8 h-8 rounded bg-surface-container-high flex items-center justify-center flex-shrink-0"
          >
            <span class="material-symbols-outlined text-sm text-outline">terminal</span>
          </div>
          <div class="flex-1 space-y-2">
            <span
              class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold"
              >System Configuration</span
            >
            <p class="text-sm text-on-surface-variant leading-relaxed italic">
              You are Zonky, a high-performance orchestration assistant. You optimize for
              precision, technical clarity, and efficiency in local inference environments.
            </p>
          </div>
        </div>

        <!-- Message Thread -->
        {#each messages as msg}
          {#if msg.role === "user"}
            <!-- User Message -->
            <div class="max-w-3xl mx-auto flex gap-6">
              <div
                class="w-8 h-8 rounded bg-primary/10 flex items-center justify-center flex-shrink-0 border border-primary/20"
              >
                <span class="material-symbols-outlined text-sm text-primary">person</span>
              </div>
              <div class="flex-1 space-y-2">
                <span
                  class="text-[10px] font-label uppercase tracking-widest text-primary font-bold"
                  >Operator</span
                >
                <p class="text-on-surface leading-relaxed">{msg.content}</p>
              </div>
            </div>
          {:else}
            <!-- Assistant Response -->
            <div class="max-w-3xl mx-auto flex gap-6">
              <div
                class="w-8 h-8 rounded bg-secondary/10 flex items-center justify-center flex-shrink-0 border border-secondary/20"
              >
                <span class="material-symbols-outlined text-sm text-secondary">smart_toy</span>
              </div>
              <div class="flex-1 space-y-4">
                <span
                  class="text-[10px] font-label uppercase tracking-widest text-secondary font-bold"
                  >Zonky Assistant</span
                >
                <div class="space-y-4 text-on-surface leading-relaxed">
                  {#each parseContent(msg.content) as block}
                    {#if block.type === "code"}
                      <!-- Styled Code Block -->
                      <div class="bg-surface-container-lowest rounded-xl border border-outline-variant/10 overflow-hidden font-mono text-[13px]">
                        <div class="bg-surface-container-low px-4 py-2 flex justify-between items-center border-b border-outline-variant/5">
                          <span class="text-on-surface-variant text-[10px] uppercase">{block.lang}</span>
                          <button
                            class="hover:text-primary transition-colors text-on-surface-variant"
                            onclick={() => copyCode(block.content)}
                          >
                            <span class="material-symbols-outlined text-sm">content_copy</span>
                          </button>
                        </div>
                        <pre class="p-4 overflow-x-auto text-on-surface-variant whitespace-pre-wrap">{block.content}</pre>
                      </div>
                    {:else}
                      <!-- Text paragraphs -->
                      {#each block.content.split("\n\n") as para}
                        {#if para.trim()}
                          <p>{para.trim()}</p>
                        {/if}
                      {/each}
                    {/if}
                  {/each}
                </div>
              </div>
            </div>
          {/if}
        {/each}

        <!-- Typing Indicator -->
        {#if sending}
          <div class="max-w-3xl mx-auto flex gap-6">
            <div
              class="w-8 h-8 rounded bg-secondary/10 flex items-center justify-center flex-shrink-0 border border-secondary/20"
            >
              <span class="material-symbols-outlined text-sm text-secondary">smart_toy</span>
            </div>
            <div class="flex-1 space-y-2">
              <span
                class="text-[10px] font-label uppercase tracking-widest text-secondary font-bold"
                >Zonky Assistant</span
              >
              <div class="text-on-surface-variant flex items-center gap-1">
                <span>Processing</span>
                <span class="token-cursor"></span>
              </div>
            </div>
          </div>
        {/if}

        <!-- Spacer for floating input -->
        <div class="h-32"></div>
      {/if}
    </div>

    <!-- Floating Input Area -->
    <div class="absolute bottom-6 left-1/2 -translate-x-1/2 w-full max-w-4xl px-6">
      <div class="glass-panel border border-outline-variant/10 rounded-2xl p-4 shadow-2xl">
        <div class="flex flex-col gap-3">
          <textarea
            class="bg-transparent border-none text-on-surface placeholder:text-on-surface-variant/50 resize-none w-full py-2 focus:ring-0 focus:outline-none"
            placeholder="Enter inference instructions..."
            rows="1"
            bind:value={input}
            onkeydown={handleKey}
            disabled={sending}
          ></textarea>
          <div class="flex items-center justify-between border-t border-outline-variant/10 pt-3">
            <div class="flex gap-2">
              {#if sending}
                <button
                  class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-container-highest text-on-surface-variant hover:text-error transition-colors"
                >
                  <span class="material-symbols-outlined text-sm">stop_circle</span>
                  <span class="text-[10px] font-label uppercase tracking-widest font-bold"
                    >Stop Generation</span
                  >
                </button>
              {/if}
              <button
                class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-container-highest text-on-surface-variant hover:text-on-surface transition-colors"
                onclick={clearChat}
              >
                <span class="material-symbols-outlined text-sm">delete_sweep</span>
                <span class="text-[10px] font-label uppercase tracking-widest font-bold"
                  >Clear Chat</span
                >
              </button>
            </div>
            <button
              class="bg-primary text-on-primary px-4 py-2 rounded-xl flex items-center gap-2 hover:shadow-[0_0_15px_rgba(76,214,255,0.4)] transition-all disabled:opacity-40"
              onclick={sendMessage}
              disabled={sending || !input.trim()}
            >
              <span class="text-sm font-bold font-headline uppercase tracking-tight"
                >Run Inference</span
              >
              <span class="material-symbols-outlined text-sm">bolt</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </section>

  <!-- Right Parameter Panel -->
  {#if showConfig}
    <aside
      class="w-80 h-full bg-surface-container-lowest overflow-y-auto no-scrollbar hidden lg:block"
    >
      <div class="p-6 space-y-8">
        <div class="flex items-center justify-between">
          <h3 class="font-headline font-bold text-sm tracking-widest uppercase">
            Inference Config
          </h3>
          <span class="material-symbols-outlined text-on-surface-variant text-sm">tune</span>
        </div>

        <!-- Parameters -->
        <div class="space-y-6">
          <!-- Temperature -->
          <div class="space-y-3">
            <div class="flex justify-between items-center">
              <label
                for="temperature-slider"
                class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold"
                >Temperature</label
              >
              <span class="text-xs font-mono text-primary">{temperature.toFixed(2)}</span>
            </div>
            <input
              id="temperature-slider"
              type="range"
              min="0"
              max="2"
              step="0.05"
              bind:value={temperature}
              class="w-full accent-primary bg-surface-container-high h-1.5 rounded-2xl appearance-none cursor-pointer"
            />
            <p class="text-[10px] text-on-surface-variant/60 leading-tight">
              Controls randomness: Lower is more deterministic.
            </p>
          </div>

          <!-- Top-P -->
          <div class="space-y-3">
            <div class="flex justify-between items-center">
              <label
                for="top-p-slider"
                class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold"
                >Top-P</label
              >
              <span class="text-xs font-mono text-primary">{topP.toFixed(2)}</span>
            </div>
            <input
              id="top-p-slider"
              type="range"
              min="0"
              max="1"
              step="0.05"
              bind:value={topP}
              class="w-full accent-primary bg-surface-container-high h-1.5 rounded-2xl appearance-none cursor-pointer"
            />
            <p class="text-[10px] text-on-surface-variant/60 leading-tight">
              Nucleus sampling threshold for token vocabulary.
            </p>
          </div>

          <!-- Repeat Penalty -->
          <div class="space-y-3">
            <div class="flex justify-between items-center">
              <label
                for="repeat-penalty-slider"
                class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold"
                >Repeat Penalty</label
              >
              <span class="text-xs font-mono text-primary">{repeatPenalty.toFixed(2)}</span>
            </div>
            <input
              id="repeat-penalty-slider"
              type="range"
              min="1"
              max="2"
              step="0.05"
              bind:value={repeatPenalty}
              class="w-full accent-primary bg-surface-container-high h-1.5 rounded-2xl appearance-none cursor-pointer"
            />
          </div>
        </div>

        <!-- Hardware Monitor Sidebar -->
        <div class="pt-6 border-t border-outline-variant/10">
          <h3
            class="font-headline font-bold text-[10px] tracking-widest uppercase text-on-surface-variant mb-4"
          >
            Hardware Monitor
          </h3>
          <div class="space-y-4">
            <div class="bg-surface-container-low p-3 rounded-lg flex items-center gap-4">
              <div class="w-1 h-8 bg-secondary rounded-2xl"></div>
              <div>
                <div class="text-[9px] font-label uppercase text-on-surface-variant">
                  GPU Memory
                </div>
                <div class="text-sm font-mono text-on-surface">{formatGB(hw.vramUsed)} / {formatGB(hw.vramTotal)} GB</div>
              </div>
            </div>
            <div class="bg-surface-container-low p-3 rounded-lg flex items-center gap-4">
              <div class="w-1 h-8 bg-tertiary rounded-2xl"></div>
              <div>
                <div class="text-[9px] font-label uppercase text-on-surface-variant">
                  Neural Engine
                </div>
                <div class="text-sm font-mono text-on-surface">
                  {hw.vramPercent > 0 ? `${hw.vramPercent}% Utilization` : '-'}
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Context Info -->
        <div class="p-4 bg-primary/5 rounded-xl border border-primary/10">
          <div class="flex gap-3">
            <span class="material-symbols-outlined text-primary text-sm">info</span>
            <p
              class="text-[10px] text-primary/80 leading-relaxed uppercase tracking-wider font-medium"
            >
              Context window currently set to 8,192 tokens.
            </p>
          </div>
        </div>
      </div>
    </aside>
  {/if}
</div>

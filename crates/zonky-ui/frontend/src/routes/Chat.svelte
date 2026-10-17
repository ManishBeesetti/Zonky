<script>
  import { onMount } from "svelte";
  import { getHardware, refresh as refreshHardware } from "../stores/hardware.svelte.js";

  const SESSIONS_STORAGE_KEY = "zonky_chat_sessions_v1";

  const SYSTEM_PRESETS = [
    {
      id: "default",
      label: "Default",
      prompt:
        "You are Zonky, a high-performance orchestration assistant. You optimize for precision, technical clarity, and efficiency in local inference environments.",
    },
    {
      id: "coder",
      label: "Coder",
      prompt:
        "You are an expert systems and software engineer. Provide clean, idiomatic, production-ready code with concise technical explanations.",
    },
    {
      id: "concise",
      label: "Concise",
      prompt:
        "Answer directly and concisely with zero filler. Prioritize bullet points and exact commands.",
    },
  ];

  let messages = $state([]);
  let input = $state("");
  let selectedModel = $state("");
  let sending = $state(false);
  let temperature = $state(0.75);
  let topP = $state(0.9);
  let repeatPenalty = $state(1.1);
  let maxTokens = $state(2048);
  let selectedPreset = $state("default");
  let systemPrompt = $state(SYSTEM_PRESETS[0].prompt);
  let showConfig = $state(true);
  let loadingModels = $state(true);
  let savedSessions = $state([]);
  let activeSessionId = $state(null);
  let exportFeedback = $state("");

  let hw = $derived(getHardware());
  let loadedModels = $derived(hw.loadedModels);

  onMount(async () => {
    try {
      const raw = localStorage.getItem(SESSIONS_STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) savedSessions = parsed;
      }
    } catch (_) {}

    try {
      const { listen } = await import("@tauri-apps/api/event");
      await listen("chat-stream-chunk", (event) => {
        const payload = event.payload;
        if (!payload || !sending) return;
        if (payload.delta) {
          const lastIdx = messages.length - 1;
          if (lastIdx >= 0 && messages[lastIdx].role === "assistant" && messages[lastIdx].streaming) {
            messages[lastIdx] = {
              ...messages[lastIdx],
              content: messages[lastIdx].content + payload.delta,
            };
          } else {
            messages = [
              ...messages,
              { role: "assistant", content: payload.delta, streaming: true },
            ];
          }
        }
      });
    } catch (_) {}
  });

  function persistSessions() {
    try {
      localStorage.setItem(SESSIONS_STORAGE_KEY, JSON.stringify(savedSessions.slice(0, 25)));
    } catch (_) {}
  }

  function saveCurrentSession() {
    if (messages.length === 0) return;
    const id = activeSessionId || `session-${Date.now()}`;
    const existingIdx = savedSessions.findIndex((s) => s.id === id);
    const sessionNum = existingIdx >= 0 ? existingIdx + 1 : savedSessions.length + 1;
    const title = `Thread #${sessionNum} · ${selectedModel || "local"}`;
    activeSessionId = id;
    const entry = {
      id,
      title,
      model: selectedModel,
      updatedAt: Date.now(),
      messages: messages.map((m) => ({
        role: m.role,
        content: m.content,
        telemetry: m.telemetry || null,
      })),
    };
    if (existingIdx >= 0) {
      savedSessions[existingIdx] = entry;
      savedSessions = [...savedSessions];
    } else {
      savedSessions = [entry, ...savedSessions];
    }
    persistSessions();
  }

  function loadSession(session) {
    if (sending) return;
    activeSessionId = session.id;
    messages = session.messages || [];
    if (session.model && loadedModels.some((m) => m.id === session.model)) {
      selectedModel = session.model;
    }
  }

  function deleteSession(sessionId) {
    savedSessions = savedSessions.filter((s) => s.id !== sessionId);
    if (activeSessionId === sessionId) {
      activeSessionId = null;
    }
    persistSessions();
  }

  function selectPreset(preset) {
    selectedPreset = preset.id;
    systemPrompt = preset.prompt;
  }

  // Auto-select first model when models become available
  $effect(() => {
    if (loadedModels.length > 0 && !selectedModel) {
      selectedModel = loadedModels[0].id;
    }
    loadingModels = false;
  });

  async function fetchLoadedModels() {
    loadingModels = true;
    await refreshHardware();
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

    const outboundMessages = [];
    if (systemPrompt.trim()) {
      outboundMessages.push({ role: "system", content: systemPrompt.trim() });
    }
    for (const m of messages) {
      if (m.role === "user" || m.role === "assistant") {
        outboundMessages.push({ role: m.role, content: m.content });
      }
    }

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const response = await invoke("chat_complete", {
        model: selectedModel,
        messages: outboundMessages,
        temperature,
        topP,
        repeatPenalty,
        maxTokens,
        stream: true,
      });

      const lastIdx = messages.length - 1;
      if (lastIdx >= 0 && messages[lastIdx].role === "assistant" && messages[lastIdx].streaming) {
        messages[lastIdx] = {
          role: "assistant",
          content: response.content || messages[lastIdx].content,
          telemetry: response.telemetry || null,
          streaming: false,
        };
      } else {
        messages = [
          ...messages,
          {
            role: "assistant",
            content: response.content,
            telemetry: response.telemetry || null,
          },
        ];
      }
      saveCurrentSession();
    } catch (e) {
      const errStr = String(e);
      const lastIdx = messages.length - 1;
      if (lastIdx >= 0 && messages[lastIdx].role === "assistant" && messages[lastIdx].streaming) {
        messages[lastIdx] = {
          ...messages[lastIdx],
          streaming: false,
          content: messages[lastIdx].content || `Error: ${errStr}`,
        };
      } else if (!errStr.toLowerCase().includes("cancelled")) {
        messages = [...messages, { role: "assistant", content: `Error: ${errStr}` }];
      }
    }
    sending = false;
  }

  async function stopGeneration() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("cancel_generation");
    } catch (_) {}
    const lastIdx = messages.length - 1;
    if (lastIdx >= 0 && messages[lastIdx].streaming) {
      messages[lastIdx] = { ...messages[lastIdx], streaming: false };
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
    activeSessionId = null;
  }

  function exportChat(format) {
    if (messages.length === 0) return;
    let content = "";
    if (format === "json") {
      content = JSON.stringify(
        {
          model: selectedModel,
          system_prompt: systemPrompt,
          parameters: { temperature, top_p: topP, repeat_penalty: repeatPenalty, max_tokens: maxTokens },
          messages,
        },
        null,
        2
      );
    } else {
      const lines = [`# Zonky Chat (${selectedModel || "local"})`, ""];
      if (systemPrompt.trim()) {
        lines.push(`> **System:** ${systemPrompt.trim()}`, "");
      }
      for (const m of messages) {
        lines.push(`### ${m.role === "user" ? "Operator" : "Zonky Assistant"}`, "", m.content, "");
      }
      content = lines.join("\n");
    }
    try {
      navigator.clipboard?.writeText(content);
    } catch (_) {}
    exportFeedback = `Exported ${format.toUpperCase()}`;
    setTimeout(() => (exportFeedback = ""), 2000);
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
      <div class="ml-auto flex items-center gap-2">
        {#if messages.length > 0}
          <button
            class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-surface-container-high text-on-surface-variant hover:text-primary transition-colors text-[10px] font-label uppercase tracking-widest font-bold"
            onclick={() => exportChat("md")}
            title="Export Markdown to clipboard"
          >
            <span class="material-symbols-outlined text-sm">download</span>
            Export MD
          </button>
          <button
            class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-surface-container-high text-on-surface-variant hover:text-primary transition-colors text-[10px] font-label uppercase tracking-widest font-bold"
            onclick={() => exportChat("json")}
            title="Export JSON to clipboard"
          >
            <span class="material-symbols-outlined text-sm">data_object</span>
            Export JSON
          </button>
        {/if}
        {#if exportFeedback}
          <span class="text-[10px] font-mono text-secondary px-2 py-1 bg-secondary/10 rounded">
            {exportFeedback}
          </span>
        {/if}
        <button
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-container-highest text-on-surface-variant hover:text-on-surface transition-colors text-[10px] font-label uppercase tracking-widest font-bold"
          onclick={fetchLoadedModels}
        >
          <span class="material-symbols-outlined text-sm">refresh</span>
          Refresh
        </button>
      </div>
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
              {systemPrompt}
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
                <div class="flex items-center gap-3">
                  <span
                    class="text-[10px] font-label uppercase tracking-widest text-secondary font-bold"
                    >Zonky Assistant</span
                  >
                  {#if msg.telemetry}
                    <span class="text-[10px] font-mono text-on-surface-variant px-2 py-0.5 rounded bg-surface-container-low">
                      {msg.telemetry.tokens_per_second.toFixed(1)} tok/s · {msg.telemetry.generated_tokens} tok · {msg.telemetry.total_latency_ms}ms
                    </span>
                  {/if}
                </div>
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
                  onclick={stopGeneration}
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

          <!-- Max Tokens -->
          <div class="space-y-3">
            <div class="flex justify-between items-center">
              <label
                for="max-tokens-slider"
                class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold"
                >Max Tokens</label
              >
              <span class="text-xs font-mono text-primary">{maxTokens}</span>
            </div>
            <input
              id="max-tokens-slider"
              type="range"
              min="128"
              max="4096"
              step="128"
              bind:value={maxTokens}
              class="w-full accent-primary bg-surface-container-high h-1.5 rounded-2xl appearance-none cursor-pointer"
            />
          </div>

          <!-- System Prompt Presets -->
          <div class="space-y-3 pt-2 border-t border-outline-variant/10">
            <div class="flex justify-between items-center">
              <span class="text-[10px] font-label uppercase tracking-widest text-on-surface-variant font-bold">
                System Prompt
              </span>
              <div class="flex gap-1">
                {#each SYSTEM_PRESETS as preset}
                  <button
                    class="px-2 py-0.5 rounded text-[9px] font-mono uppercase transition-colors {selectedPreset === preset.id ? 'bg-primary text-on-primary font-bold' : 'bg-surface-container-high text-on-surface-variant hover:text-on-surface'}"
                    onclick={() => selectPreset(preset)}
                  >
                    {preset.label}
                  </button>
                {/each}
              </div>
            </div>
            <textarea
              rows="3"
              bind:value={systemPrompt}
              oninput={() => (selectedPreset = "custom")}
              placeholder="Custom system instructions..."
              class="w-full bg-surface-container-low rounded-lg p-2.5 text-xs text-on-surface border border-outline-variant/20 focus:border-primary focus:outline-none resize-none"
            ></textarea>
          </div>
        </div>

        <!-- Saved Sessions -->
        {#if savedSessions.length > 0}
          <div class="pt-6 border-t border-outline-variant/10">
            <div class="flex items-center justify-between mb-3">
              <h3 class="font-headline font-bold text-[10px] tracking-widest uppercase text-on-surface-variant">
                Saved Sessions
              </h3>
              <button
                class="text-[10px] font-mono text-primary hover:underline"
                onclick={clearChat}
              >
                + New
              </button>
            </div>
            <div class="space-y-1.5 max-h-40 overflow-y-auto no-scrollbar">
              {#each savedSessions as s}
                <div class="flex items-center justify-between px-3 py-2 rounded-lg bg-surface-container-low hover:bg-surface-container-high transition-colors group">
                  <button
                    class="text-left text-xs text-on-surface truncate flex-1"
                    onclick={() => loadSession(s)}
                  >
                    {s.title} ({s.messages?.length || 0})
                  </button>
                  <button
                    class="text-on-surface-variant/50 hover:text-error ml-2"
                    onclick={() => deleteSession(s.id)}
                    title="Delete session"
                  >
                    <span class="material-symbols-outlined text-xs">close</span>
                  </button>
                </div>
              {/each}
            </div>
          </div>
        {/if}

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

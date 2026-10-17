/**
 * Mock responses for Tauri invoke commands.
 * Used by Playwright tests to simulate the Tauri backend.
 */

export const MOCK_DEVICES = [
  {
    name: "AMD Radeon Strix Halo",
    vendor: "AMD",
    vram_total: 103079215104,
    vram_free: 96636764160,
    is_gpu: true,
  },
  {
    name: "CPU",
    vendor: "AMD",
    vram_total: 0,
    vram_free: 0,
    is_gpu: false,
  },
];

export const MOCK_SYSTEM_INFO = {
  devices: MOCK_DEVICES,
  total_memory: 103079215104,
  available_memory: 85899345920,
};

export const MOCK_SETUP_STATUS = {
  gpu_vendor: "AMD",
  gpu_name: "AMD Radeon Strix Halo",
  compute_ready: true,
  needs_reboot: false,
  deps: [
    { name: "rocm", description: "AMD ROCm compute stack", installed: true },
  ],
  summary: "All dependencies met",
};

export const MOCK_LOCAL_MODELS = [
  {
    id: "test-model-1",
    repo_id: "TestOrg/SmallModel-7B-GGUF",
    filename: "smallmodel-q4_k_m.gguf",
    size: 4294967296,
    path: "/home/user/.cache/zonky/models/TestOrg--SmallModel-7B-GGUF/smallmodel-q4_k_m.gguf",
    compatible_backends: ["candle"],
  },
  {
    id: "test-model-2",
    repo_id: "TestOrg/TinyModel-1B-GGUF",
    filename: "tinymodel-q8_0.gguf",
    size: 1073741824,
    path: "/home/user/.cache/zonky/models/TestOrg--TinyModel-1B-GGUF/tinymodel-q8_0.gguf",
    compatible_backends: ["candle"],
  },
];

export const MOCK_LOADED_MODELS = [
  {
    id: "test-model-1",
    backend: "candle",
    vram_usage: 4724464025,
    loaded: true,
  },
];

export const MOCK_SEARCH_RESULTS = [
  {
    model_id: "bartowski/Llama-3.2-3B-Instruct-GGUF",
    author: "bartowski",
    downloads: 123456,
    likes: 4200,
  },
  {
    model_id: "TestOrg/SmallModel-7B-GGUF",
    author: "TestOrg",
    downloads: 50000,
    likes: 1200,
  },
  {
    model_id: "TestOrg/TinyModel-1B-GGUF",
    author: "TestOrg",
    downloads: 25000,
    likes: 800,
  },
];

export const MOCK_GGUF_FILES = [
  {
    filename: "llama-3.2-3b-instruct-q4_k_m.gguf",
    size: 1610612736,
    quantization: "Q4_K_M",
    estimated_vram_bytes: 1852204646,
    fits_in_memory: true,
    recommended: true,
    reason: "Recommended balanced quantization for your hardware",
  },
  {
    filename: "llama-3.2-3b-instruct-q8_0.gguf",
    size: 3221225472,
    quantization: "Q8_0",
    estimated_vram_bytes: 3704409292,
    fits_in_memory: true,
    recommended: false,
    reason: "Fits in available memory (~3.5 GB required)",
  },
  {
    filename: "model-q4_k_m.gguf",
    size: 4294967296,
    quantization: "Q4_K_M",
    estimated_vram_bytes: 4939212390,
    fits_in_memory: true,
    recommended: false,
    reason: "Fits in available memory (~4.6 GB required)",
  },
  {
    filename: "model-q8_0.gguf",
    size: 8589934592,
    quantization: "Q8_0",
    estimated_vram_bytes: 9878424780,
    fits_in_memory: true,
    recommended: false,
    reason: "Fits in available memory (~9.2 GB required)",
  },
];

export const MOCK_TELEMETRY = {
  total_requests: 2,
  total_tokens_generated: 84,
  avg_tokens_per_second: 42.5,
  avg_latency_ms: 185,
  recent_records: [],
};

export const MOCK_CHAT_RESPONSE = {
  content: "Hello! I'm a test response from the mocked inference backend.",
  model: "test-model-1",
  tokens_used: 42,
  telemetry: {
    timestamp_ms: 1730000000000,
    model_id: "test-model-1",
    backend_name: "candle",
    prompt_tokens: 18,
    generated_tokens: 42,
    tokens_per_second: 42.5,
    total_latency_ms: 185,
  },
};

export const MOCK_CONFIG = {
  cache_dir: "/home/user/.cache/zonky",
  default_backend: "llamacpp",
  server_host: "127.0.0.1",
  server_port: 8080,
  api_key: null,
  max_vram_percent: 90,
  auto_evict: true,
};

export const MOCK_SERVER_STATUS = {
  running: false,
  host: "127.0.0.1",
  port: 8080,
  url: "http://127.0.0.1:8080",
};

/**
 * Build the invoke handler map used by the Tauri mock.
 */
export function buildInvokeHandler(overrides = {}) {
  const handlers = {
    get_devices: () => MOCK_DEVICES,
    get_system_info: () => MOCK_SYSTEM_INFO,
    get_setup_status: () => MOCK_SETUP_STATUS,
    get_config: () => MOCK_CONFIG,
    save_config: ({ config }) => config,
    reset_config: () => MOCK_CONFIG,
    clear_model_cache: () => 2,
    list_local_models: () => MOCK_LOCAL_MODELS,
    list_loaded_models: () => MOCK_LOADED_MODELS,
    search_models: () => MOCK_SEARCH_RESULTS,
    list_gguf_files: () => MOCK_GGUF_FILES,
    recommend_model_files: () => MOCK_GGUF_FILES,
    load_model: ({ modelId }) => `Model '${modelId}' loaded successfully`,
    unload_model: ({ modelId }) => `Model '${modelId}' unloaded`,
    chat_complete: () => MOCK_CHAT_RESPONSE,
    cancel_generation: () => true,
    pull_model: () => "Downloaded",
    cancel_download: () => true,
    delete_model: () => "Deleted",
    get_telemetry: () => MOCK_TELEMETRY,
    clear_telemetry: () => null,
    ...overrides,
  };
  return handlers;
}

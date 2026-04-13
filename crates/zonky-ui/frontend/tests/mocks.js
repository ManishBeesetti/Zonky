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
  { filename: "llama-3.2-3b-instruct-q4_k_m.gguf", size: 1610612736 },
  { filename: "llama-3.2-3b-instruct-q8_0.gguf", size: 3221225472 },
  { filename: "model-q4_k_m.gguf", size: 4294967296 },
  { filename: "model-q8_0.gguf", size: 8589934592 },
];

export const MOCK_CHAT_RESPONSE = {
  content: "Hello! I'm a test response from the mocked inference backend.",
  model: "test-model-1",
  tokens_used: 42,
};

export const MOCK_CONFIG = {
  cache_dir: "/home/user/.cache/zonky",
  auto_evict: true,
  max_vram_percent: 90,
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
    list_local_models: () => MOCK_LOCAL_MODELS,
    list_loaded_models: () => MOCK_LOADED_MODELS,
    search_models: () => MOCK_SEARCH_RESULTS,
    list_gguf_files: () => MOCK_GGUF_FILES,
    load_model: ({ modelId }) => `Model '${modelId}' loaded successfully`,
    unload_model: ({ modelId }) => `Model '${modelId}' unloaded`,
    chat_complete: () => MOCK_CHAT_RESPONSE,
    pull_model: () => "Downloaded",
    delete_model: () => "Deleted",
    ...overrides,
  };
  return handlers;
}

import { test as base } from "@playwright/test";
import {
  MOCK_DEVICES,
  MOCK_SYSTEM_INFO,
  MOCK_SETUP_STATUS,
  MOCK_CONFIG,
  MOCK_LOCAL_MODELS,
  MOCK_LOADED_MODELS,
  MOCK_SEARCH_RESULTS,
  MOCK_GGUF_FILES,
  MOCK_CHAT_RESPONSE,
  MOCK_SERVER_STATUS,
  MOCK_TELEMETRY,
} from "./mocks.js";

/**
 * Extended test fixture that injects Tauri IPC mocks before each test.
 * Uses @tauri-apps/api's internal `window.__TAURI_INTERNALS__` bridge.
 */
export const test = base.extend({
  page: async ({ page }, use) => {
    const mockData = {
      get_devices: MOCK_DEVICES,
      get_system_info: MOCK_SYSTEM_INFO,
      get_setup_status: MOCK_SETUP_STATUS,
      get_config: MOCK_CONFIG,
      reset_config: MOCK_CONFIG,
      clear_model_cache: 2,
      list_local_models: MOCK_LOCAL_MODELS,
      list_loaded_models: MOCK_LOADED_MODELS,
      search_models: MOCK_SEARCH_RESULTS,
      list_gguf_files: MOCK_GGUF_FILES,
      recommend_model_files: MOCK_GGUF_FILES,
      chat_complete: MOCK_CHAT_RESPONSE,
      cancel_generation: true,
      load_model: "Model loaded successfully",
      unload_model: "Model unloaded",
      pull_model: "Downloaded",
      cancel_download: true,
      delete_model: "Deleted",
      get_server_status: MOCK_SERVER_STATUS,
      start_server: { ...MOCK_SERVER_STATUS, running: true },
      stop_server: "Server stopped",
      get_telemetry: MOCK_TELEMETRY,
      clear_telemetry: null,
    };

    await page.addInitScript((data) => {
      window.isTauri = true;
      window.__TAURI_INVOKE_LOG__ = [];

      window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
      window.__TAURI_INTERNALS__.invoke = async (cmd, args, options) => {
        window.__TAURI_INVOKE_LOG__.push({ cmd, args });

        if (cmd.startsWith("plugin:")) {
          return null;
        }
        if (window.__TAURI_MOCK_OVERRIDES__ && cmd in window.__TAURI_MOCK_OVERRIDES__) {
          const val = window.__TAURI_MOCK_OVERRIDES__[cmd];
          return typeof val === "function" ? val(args) : val;
        }
        if (cmd === "save_config") {
          return args?.config || data.get_config;
        }
        if (cmd === "load_model") {
          await new Promise((r) => setTimeout(r, 150));
          return data.load_model;
        }
        if (cmd === "load_model") {
          await new Promise((r) => setTimeout(r, 150));
          return data.load_model;
        }
        if (cmd === "list_gguf_files") {
          if (args && args.repoId === "bartowski/Llama-3.2-3B-Instruct-GGUF") {
            return [
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
            ];
          }
          return data.list_gguf_files;
        }
        if (cmd in data) {
          return data[cmd];
        }
        console.warn(`[Tauri Mock] Unhandled command: ${cmd}`, args);
        return null;
      };
      window.__TAURI_INTERNALS__.metadata = {
        currentWebview: { label: "main" },
        currentWindow: { label: "main" },
      };
      window.__TAURI_INTERNALS__.transformCallback = (callback) => {
        return Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
      };
    }, mockData);

    await use(page);
  },
});

export { expect } from "@playwright/test";

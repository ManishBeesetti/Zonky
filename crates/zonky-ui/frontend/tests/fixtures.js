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
} from "./mocks.js";

/**
 * Extended test fixture that injects Tauri IPC mocks before each test.
 * Uses @tauri-apps/api's internal `window.__TAURI_INTERNALS__` bridge.
 */
export const test = base.extend({
  page: async ({ page }, use) => {
    // Build a plain data map that can be serialized to the browser context
    const mockData = {
      get_devices: MOCK_DEVICES,
      get_system_info: MOCK_SYSTEM_INFO,
      get_setup_status: MOCK_SETUP_STATUS,
      get_config: MOCK_CONFIG,
      list_local_models: MOCK_LOCAL_MODELS,
      list_loaded_models: MOCK_LOADED_MODELS,
      search_models: MOCK_SEARCH_RESULTS,
      // list_gguf_files is handled below for repo-specific results
      chat_complete: MOCK_CHAT_RESPONSE,
      load_model: "Model loaded successfully",
      unload_model: "Model unloaded",
      pull_model: "Downloaded",
      delete_model: "Deleted",
      get_server_status: MOCK_SERVER_STATUS,
      start_server: { ...MOCK_SERVER_STATUS, running: true },
      stop_server: "Server stopped",
    };

    // Inject mock before any page scripts run
    await page.addInitScript((data) => {
      // Mark as Tauri environment
      window.isTauri = true;

      window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ || {};
      window.__TAURI_INTERNALS__.invoke = async (cmd, args, options) => {
        // Handle event plugin commands (listen/unlisten)
        if (cmd.startsWith("plugin:")) {
          return null;
        }
        if (cmd === "list_gguf_files") {
          if (args && args.repoId === "bartowski/Llama-3.2-3B-Instruct-GGUF") {
            return [
              { filename: "llama-3.2-3b-instruct-q4_k_m.gguf", size: 1610612736 },
              { filename: "llama-3.2-3b-instruct-q8_0.gguf", size: 3221225472 },
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

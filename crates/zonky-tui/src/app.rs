use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;
use zonky_core::types::{GenerationRequest, Message, Role};
use zonky_core::{ModelManager, ZonkyConfig};

/// Active tab in the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Models,
    Chat,
    Dashboard,
    Settings,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Models, Tab::Chat, Tab::Dashboard, Tab::Settings]
    }

    pub fn title(&self) -> &str {
        match self {
            Tab::Models => "Models",
            Tab::Chat => "Chat",
            Tab::Dashboard => "Dashboard",
            Tab::Settings => "Settings",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Tab::Models => 0,
            Tab::Chat => 1,
            Tab::Dashboard => 2,
            Tab::Settings => 3,
        }
    }
}

/// Chat message for the TUI
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Async events emitted by background TUI tasks
#[derive(Debug)]
pub enum AppEvent {
    SearchCompleted(Result<Vec<String>, String>),
    ModelLoaded(Result<String, String>),
    ModelUnloaded(Result<String, String>),
    ChatDelta(String),
    ChatCompleted(Result<(), String>),
}

/// Main TUI application state
pub struct App {
    pub active_tab: Tab,
    pub input_mode: bool,
    pub input_buffer: String,
    pub chat_messages: Vec<ChatMessage>,
    pub search_query: String,
    pub search_results: Vec<String>,
    pub local_models: Vec<String>,
    pub loaded_models: Vec<String>,
    pub selected_local_index: usize,
    pub selected_model: Option<String>,
    pub is_generating: bool,
    pub is_searching: bool,
    pub status_message: String,
    pub manager: Arc<ModelManager>,
    pub config: ZonkyConfig,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl App {
    pub fn new(config: ZonkyConfig) -> anyhow::Result<Self> {
        let manager = Arc::new(ModelManager::new(config.clone())?);
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Load local model list
        let local_models: Vec<String> = manager
            .hub()
            .list_local_models()
            .unwrap_or_default()
            .iter()
            .map(|m| m.id.clone())
            .collect();

        let selected_model = local_models.first().cloned();

        Ok(Self {
            active_tab: Tab::Models,
            input_mode: false,
            input_buffer: String::new(),
            chat_messages: Vec::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            local_models,
            loaded_models: Vec::new(),
            selected_local_index: 0,
            selected_model,
            is_generating: false,
            is_searching: false,
            status_message:
                "Welcome to Zonky TUI! Press Tab to switch tabs, / to search, Enter to load model."
                    .to_string(),
            manager,
            config,
            event_tx,
            event_rx,
        })
    }

    pub fn next_tab(&mut self) {
        let tabs = Tab::all();
        let current = self.active_tab.index();
        self.active_tab = tabs[(current + 1) % tabs.len()];
    }

    pub fn prev_tab(&mut self) {
        let tabs = Tab::all();
        let current = self.active_tab.index();
        self.active_tab = tabs[(current + tabs.len() - 1) % tabs.len()];
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.active_tab {
            Tab::Chat => self.handle_chat_key(key),
            Tab::Models => self.handle_models_key(key),
            _ => {}
        }
    }

    fn handle_chat_key(&mut self, key: KeyEvent) {
        if self.input_mode {
            match key.code {
                KeyCode::Enter => {
                    if !self.input_buffer.trim().is_empty() && !self.is_generating {
                        let msg = self.input_buffer.drain(..).collect::<String>();
                        self.submit_chat_message(msg);
                    }
                }
                KeyCode::Char(c) => self.input_buffer.push(c),
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Esc => {
                    self.input_mode = false;
                    if self.is_generating {
                        let manager = self.manager.clone();
                        tokio::spawn(async move {
                            manager.cancel_generation().await;
                        });
                        self.is_generating = false;
                        self.status_message = "Generation cancelled.".to_string();
                    }
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('i') | KeyCode::Enter => self.input_mode = true,
                KeyCode::Char('c') => {
                    self.chat_messages.clear();
                    self.status_message = "Cleared chat history.".to_string();
                }
                KeyCode::Esc if self.is_generating => {
                    let manager = self.manager.clone();
                    tokio::spawn(async move {
                        manager.cancel_generation().await;
                    });
                    self.is_generating = false;
                    self.status_message = "Generation cancelled.".to_string();
                }
                _ => {}
            }
        }
    }

    pub fn submit_chat_message(&mut self, msg: String) {
        self.chat_messages.push(ChatMessage {
            role: "user".to_string(),
            content: msg,
        });

        let Some(model_id) = self
            .selected_model
            .clone()
            .or_else(|| self.loaded_models.first().cloned())
        else {
            self.status_message =
                "No model selected or loaded. Go to Models tab and press Enter on a local model."
                    .to_string();
            self.chat_messages.push(ChatMessage {
                role: "system".to_string(),
                content: "No model is loaded. Please load a model in the Models tab first."
                    .to_string(),
            });
            return;
        };

        self.is_generating = true;
        self.status_message = format!("Generating response with '{model_id}'...");
        self.chat_messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: String::new(),
        });

        let messages: Vec<Message> = self
            .chat_messages
            .iter()
            .filter(|m| !m.content.is_empty() && (m.role == "user" || m.role == "assistant"))
            .map(|m| Message {
                role: if m.role == "user" {
                    Role::User
                } else {
                    Role::Assistant
                },
                content: m.content.clone(),
            })
            .collect();

        let request = GenerationRequest {
            model: model_id.clone(),
            messages,
            stream: true,
            ..GenerationRequest::default()
        };

        let manager = self.manager.clone();
        let tx = self.event_tx.clone();
        let default_backend = self.config.default_backend.clone();
        let default_device = self.config.default_device.clone();

        tokio::spawn(async move {
            // Auto-load if not yet loaded
            let is_loaded = manager
                .list_loaded_models()
                .await
                .iter()
                .any(|m| m.id == model_id);
            if !is_loaded {
                if let Err(e) = manager
                    .load_model(&model_id, default_backend, default_device)
                    .await
                {
                    let _ = tx.send(AppEvent::ChatCompleted(Err(e.to_string())));
                    return;
                }
            }

            match manager.generate_stream(&model_id, &request).await {
                Ok(mut rx) => {
                    while let Some(chunk_res) = rx.recv().await {
                        match chunk_res {
                            Ok(chunk) => {
                                let text = chunk.delta_text();
                                if !text.is_empty() {
                                    let _ = tx.send(AppEvent::ChatDelta(text.to_string()));
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(AppEvent::ChatCompleted(Err(e.to_string())));
                                return;
                            }
                        }
                    }
                    let _ = tx.send(AppEvent::ChatCompleted(Ok(())));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::ChatCompleted(Err(e.to_string())));
                }
            }
        });
    }

    fn handle_models_key(&mut self, key: KeyEvent) {
        if self.input_mode {
            match key.code {
                KeyCode::Enter => {
                    let query = self.search_query.trim().to_string();
                    self.input_mode = false;
                    if !query.is_empty() {
                        self.trigger_search(query);
                    }
                }
                KeyCode::Char(c) => self.search_query.push(c),
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Esc => self.input_mode = false,
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('/') => {
                    self.input_mode = true;
                    self.search_query.clear();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.local_models.is_empty() {
                        self.selected_local_index =
                            (self.selected_local_index + 1) % self.local_models.len();
                        self.selected_model =
                            self.local_models.get(self.selected_local_index).cloned();
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !self.local_models.is_empty() {
                        self.selected_local_index =
                            (self.selected_local_index + self.local_models.len() - 1)
                                % self.local_models.len();
                        self.selected_model =
                            self.local_models.get(self.selected_local_index).cloned();
                    }
                }
                KeyCode::Enter | KeyCode::Char('l') => {
                    if let Some(model_id) =
                        self.local_models.get(self.selected_local_index).cloned()
                    {
                        self.selected_model = Some(model_id.clone());
                        self.status_message = format!("Loading model '{model_id}'...");
                        let manager = self.manager.clone();
                        let tx = self.event_tx.clone();
                        let backend = self.config.default_backend;
                        let device = self.config.default_device.clone();
                        tokio::spawn(async move {
                            let res = manager
                                .load_model(&model_id, backend, device)
                                .await
                                .map(|()| model_id)
                                .map_err(|e| e.to_string());
                            let _ = tx.send(AppEvent::ModelLoaded(res));
                        });
                    }
                }
                KeyCode::Char('u') => {
                    if let Some(model_id) = self.selected_model.clone() {
                        self.status_message = format!("Unloading model '{model_id}'...");
                        let manager = self.manager.clone();
                        let tx = self.event_tx.clone();
                        tokio::spawn(async move {
                            let res = manager
                                .unload_model(&model_id)
                                .await
                                .map(|()| model_id)
                                .map_err(|e| e.to_string());
                            let _ = tx.send(AppEvent::ModelUnloaded(res));
                        });
                    }
                }
                _ => {}
            }
        }
    }

    pub fn trigger_search(&mut self, query: String) {
        self.is_searching = true;
        self.status_message = format!("Searching HuggingFace for '{query}'...");
        let manager = self.manager.clone();
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let res = manager
                .hub()
                .search_models(&query, 15)
                .await
                .map(|models| {
                    models
                        .into_iter()
                        .map(|m| format!("{} (↓ {})", m.model_id, m.downloads.unwrap_or(0)))
                        .collect::<Vec<_>>()
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::SearchCompleted(res));
        });
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SearchCompleted(Ok(results)) => {
                self.is_searching = false;
                let count = results.len();
                self.search_results = results;
                self.status_message = format!("Found {count} models on HuggingFace.");
            }
            AppEvent::SearchCompleted(Err(err)) => {
                self.is_searching = false;
                self.status_message = format!("Search failed: {err}");
            }
            AppEvent::ModelLoaded(Ok(model_id)) => {
                self.selected_model = Some(model_id.clone());
                if !self.loaded_models.contains(&model_id) {
                    self.loaded_models.push(model_id.clone());
                }
                self.status_message = format!("Loaded model '{model_id}'.");
            }
            AppEvent::ModelLoaded(Err(err)) => {
                self.status_message = format!("Failed to load model: {err}");
            }
            AppEvent::ModelUnloaded(Ok(model_id)) => {
                self.loaded_models.retain(|m| m != &model_id);
                self.status_message = format!("Unloaded model '{model_id}'.");
            }
            AppEvent::ModelUnloaded(Err(err)) => {
                self.status_message = format!("Failed to unload model: {err}");
            }
            AppEvent::ChatDelta(delta) => {
                if let Some(last) = self.chat_messages.last_mut() {
                    if last.role == "assistant" {
                        last.content.push_str(&delta);
                        return;
                    }
                }
                self.chat_messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: delta,
                });
            }
            AppEvent::ChatCompleted(Ok(())) => {
                self.is_generating = false;
                self.status_message = "Response complete.".to_string();
            }
            AppEvent::ChatCompleted(Err(err)) => {
                self.is_generating = false;
                self.status_message = format!("Generation error: {err}");
                if let Some(last) = self.chat_messages.last_mut() {
                    if last.role == "assistant" && last.content.is_empty() {
                        last.role = "system".to_string();
                        last.content = format!("Error: {err}");
                    }
                }
            }
        }
    }

    /// Process ongoing async operations
    pub async fn tick(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            self.apply_event(event);
        }

        if let Ok(models) = self.manager.hub().list_local_models() {
            self.local_models = models.into_iter().map(|m| m.id).collect();
            if self.selected_model.is_none() {
                self.selected_model = self.local_models.first().cloned();
            }
        }

        self.loaded_models = self
            .manager
            .list_loaded_models()
            .await
            .into_iter()
            .map(|m| m.id)
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let mut cfg = ZonkyConfig::default();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        cfg.cache_dir =
            std::env::temp_dir().join(format!("zonky_tui_test_{}_{}", std::process::id(), nanos));
        App::new(cfg).unwrap()
    }

    #[test]
    fn updates_search_results_and_chat_deltas_from_events() {
        let mut app = test_app();
        app.apply_event(AppEvent::SearchCompleted(Ok(vec![
            "Bartowski/Qwen2.5-7B-GGUF (↓ 1000)".to_string(),
        ])));
        assert_eq!(app.search_results.len(), 1);
        assert!(app.status_message.contains("Found 1 models"));

        app.chat_messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: String::new(),
        });
        app.apply_event(AppEvent::ChatDelta("Hello ".to_string()));
        app.apply_event(AppEvent::ChatDelta("world!".to_string()));
        app.apply_event(AppEvent::ChatCompleted(Ok(())));

        assert_eq!(app.chat_messages.last().unwrap().content, "Hello world!");
        assert!(!app.is_generating);
    }

    #[test]
    fn handles_chat_submission_without_loaded_model_gracefully() {
        let mut app = test_app();
        app.selected_model = None;
        app.loaded_models.clear();
        app.submit_chat_message("Hello".to_string());
        assert!(!app.is_generating);
        assert_eq!(app.chat_messages.len(), 2);
        assert_eq!(app.chat_messages[1].role, "system");
    }
}

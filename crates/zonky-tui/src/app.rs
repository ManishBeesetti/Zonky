use crossterm::event::{KeyCode, KeyEvent};
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
pub struct ChatMessage {
    pub role: String,
    pub content: String,
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
    pub selected_model: Option<String>,
    pub status_message: String,
    pub manager: ModelManager,
    pub config: ZonkyConfig,
}

impl App {
    pub fn new(config: ZonkyConfig) -> anyhow::Result<Self> {
        let manager = ModelManager::new(config.clone())?;

        // Load local model list
        let local_models: Vec<String> = manager
            .hub()
            .list_local_models()
            .unwrap_or_default()
            .iter()
            .map(|m| m.id.clone())
            .collect();

        Ok(Self {
            active_tab: Tab::Models,
            input_mode: false,
            input_buffer: String::new(),
            chat_messages: Vec::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            local_models,
            selected_model: None,
            status_message: "Welcome to Zonky TUI! Press Tab to switch tabs.".to_string(),
            manager,
            config,
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
                    if !self.input_buffer.is_empty() {
                        let msg = self.input_buffer.drain(..).collect::<String>();
                        self.chat_messages.push(ChatMessage {
                            role: "user".to_string(),
                            content: msg,
                        });
                        // TODO: Trigger async generation
                        self.status_message = "Generating response...".to_string();
                    }
                }
                KeyCode::Char(c) => self.input_buffer.push(c),
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Esc => self.input_mode = false,
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('i') | KeyCode::Enter => self.input_mode = true,
                _ => {}
            }
        }
    }

    fn handle_models_key(&mut self, key: KeyEvent) {
        if self.input_mode {
            match key.code {
                KeyCode::Enter => {
                    let query = self.search_query.clone();
                    if !query.is_empty() {
                        self.status_message = format!("Searching for '{query}'...");
                        // TODO: Trigger async search
                    }
                    self.input_mode = false;
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
                _ => {}
            }
        }
    }

    /// Process ongoing async operations
    pub async fn tick(&mut self) {
        // Refresh local models periodically
    }
}

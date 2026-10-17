//! O estado da tela, em serde: é o que o `ui.inspect` do canal de controle
//! devolve e o que os comandos `ui.*` mudam. Nada aqui é dado do motor —
//! só o que está aberto, selecionado e como.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::theme::Appearance;

/// As abas da thread (ou da tela do repositório), na ordem de `Ctrl+1…6`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MainTab {
    Chat,
    History,
    Tree,
    Reflog,
    Stats,
    Terminal,
}

pub const MAIN_TABS: [MainTab; 6] = [MainTab::Chat, MainTab::History, MainTab::Tree, MainTab::Reflog, MainTab::Stats, MainTab::Terminal];

impl MainTab {
    pub fn id(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::History => "history",
            Self::Tree => "tree",
            Self::Reflog => "reflog",
            Self::Stats => "stats",
            Self::Terminal => "terminal",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        MAIN_TABS.into_iter().find(|t| t.id() == s)
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Chat => "mainTabs.chat",
            Self::History => "mainTabs.history",
            Self::Tree => "mainTabs.tree",
            Self::Reflog => "mainTabs.reflog",
            Self::Stats => "mainTabs.stats",
            Self::Terminal => "mainTabs.terminal",
        }
    }

    pub fn is_git(self) -> bool {
        !matches!(self, Self::Chat | Self::Terminal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsTab {
    Workspaces,
    Appearance,
    Skills,
    Agents,
    Mcp,
    Accounts,
    Providers,
    Defaults,
    Git,
    Integrations,
    Shortcuts,
    About,
}

pub const SETTINGS_TABS: [SettingsTab; 12] = [
    SettingsTab::Workspaces,
    SettingsTab::Appearance,
    SettingsTab::Defaults,
    SettingsTab::Accounts,
    SettingsTab::Providers,
    SettingsTab::Skills,
    SettingsTab::Agents,
    SettingsTab::Mcp,
    SettingsTab::Git,
    SettingsTab::Integrations,
    SettingsTab::Shortcuts,
    SettingsTab::About,
];

impl SettingsTab {
    pub fn id(self) -> &'static str {
        match self {
            Self::Workspaces => "workspaces",
            Self::Appearance => "appearance",
            Self::Skills => "skills",
            Self::Agents => "agents",
            Self::Mcp => "mcp",
            Self::Accounts => "accounts",
            Self::Providers => "providers",
            Self::Defaults => "defaults",
            Self::Git => "git",
            Self::Integrations => "integrations",
            Self::Shortcuts => "shortcuts",
            Self::About => "about",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        SETTINGS_TABS.into_iter().find(|t| t.id() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TreeMode {
    Project,
    Recent,
}

/// O que a aba Histórico mostra: o grafo do git ou a revisão do agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryView {
    Graph,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewBase {
    Upstream,
    Turn,
    Head,
}

impl ReviewBase {
    pub fn id(self) -> &'static str {
        match self {
            Self::Upstream => "upstream",
            Self::Turn => "turn",
            Self::Head => "head",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct UiState {
    pub tab: MainTab,
    /// thread aberta
    pub selected: Option<String>,
    /// repositório aberto na tela do repositório (sem thread)
    pub repo_project: Option<String>,
    pub sidebar_open: bool,
    pub sidebar_width: f32,
    pub palette_open: bool,
    pub palette_query: String,
    pub settings_open: bool,
    pub settings_tab: SettingsTab,
    /// workspace em edição nas configurações
    pub settings_workspace: Option<String>,
    pub new_workspace_open: bool,
    /// escolha de quais workspaces abrir (na abertura do app ou pelo "+")
    pub picking_workspaces: bool,
    pub open_workspaces: Vec<String>,
    pub show_archived: bool,
    pub tree_mode: TreeMode,
    pub filter: String,
    pub collapsed: HashMap<String, bool>,
    pub show_all: HashMap<String, bool>,
    pub history_view: HistoryView,
    pub review_base: ReviewBase,
    /// arquivo aberto na revisão
    pub review_file: Option<String>,
    pub appearance: Appearance,
    /// atalhos trocados pelo usuário: id do comando → atalho
    pub shortcuts: HashMap<String, String>,
    /// diálogo aberto (`confirm-delete`, `about`…), para o inspect
    pub dialog: Option<String>,
    pub shortcuts_help: bool,
    /// a lista do que pede atenção em todos os workspaces (barra de título)
    pub activity_open: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            tab: MainTab::Chat,
            selected: None,
            repo_project: None,
            sidebar_open: true,
            sidebar_width: 264.0,
            palette_open: false,
            palette_query: String::new(),
            settings_open: false,
            settings_tab: SettingsTab::Workspaces,
            settings_workspace: None,
            new_workspace_open: false,
            picking_workspaces: false,
            open_workspaces: vec![],
            show_archived: false,
            tree_mode: TreeMode::Project,
            filter: String::new(),
            collapsed: HashMap::new(),
            show_all: HashMap::new(),
            history_view: HistoryView::Graph,
            review_base: ReviewBase::Upstream,
            review_file: None,
            appearance: Appearance::default(),
            shortcuts: HashMap::new(),
            dialog: None,
            shortcuts_help: false,
            activity_open: false,
        }
    }
}

/// Preferências que vão para o motor (`prefs.set`) e voltam na abertura:
/// assim a versão web e o desktop lembram o mesmo.
pub const PERSISTED: [&str; 7] =
    ["ui.appearance", "ui.openWorkspaces", "ui.sidebarOpen", "ui.sidebarWidth", "ui.treeMode", "ui.shortcuts", "ui.collapsed"];

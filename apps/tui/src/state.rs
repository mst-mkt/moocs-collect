#[derive(Debug, Clone)]
pub enum AppState {
    Login(LoginState),
    Main(MainState),
}

#[derive(Debug, Clone)]
pub struct LoginState {
    pub phase: LoginPhase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginPhase {
    Input,
    Authenticating,
}

#[derive(Debug, Clone)]
pub struct MainState {
    pub active_tab: Tab,
    pub selector_phase: SelectorPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Selector,
    Download,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorPhase {
    Idle,
    LoadingCourses,
    Ready,
    EnqueuePending,
}

impl AppState {
    pub const fn can_handle_input(&self) -> bool {
        !matches!(
            self,
            Self::Login(LoginState {
                phase: LoginPhase::Authenticating
            })
        )
    }

    pub const fn loading_message(&self) -> Option<&'static str> {
        match self {
            Self::Login(LoginState {
                phase: LoginPhase::Authenticating,
            }) => Some("ログイン中..."),
            Self::Main(MainState {
                selector_phase: SelectorPhase::LoadingCourses,
                ..
            }) => Some("科目を取得中..."),
            Self::Main(MainState {
                selector_phase: SelectorPhase::EnqueuePending,
                ..
            }) => Some("データを取得中..."),
            _ => None,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::Login(LoginState {
            phase: LoginPhase::Input,
        })
    }
}

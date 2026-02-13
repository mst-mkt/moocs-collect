use collect::error::CollectError;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum TuiError {
    #[error("認証に失敗しました: {reason}")]
    Authentication { reason: String },

    #[error("ネットワークエラー: {message}")]
    Network { message: String },

    #[error("データの取得に失敗しました: {resource}")]
    DataFetch { resource: String, details: String },
}

impl TuiError {
    pub fn user_message(&self) -> String {
        match self {
            Self::Authentication { reason } => {
                format!(
                    "認証に失敗しました\n\n{reason}\n\nユーザー名とパスワードを確認してください。"
                )
            }
            Self::Network { message } => {
                format!(
                    "ネットワークエラーが発生しました\n\n{message}\n\nインターネット接続を確認してください。"
                )
            }
            Self::DataFetch { resource, details } => {
                format!("{resource}の取得に失敗しました\n\n{details}")
            }
        }
    }

    pub fn from_collect(e: CollectError, context: &str) -> Self {
        match e {
            CollectError::Authentication { reason } => Self::Authentication { reason },
            CollectError::Network { .. } => Self::Network {
                message: e.to_string(),
            },
            _ => Self::DataFetch {
                resource: context.to_string(),
                details: e.to_string(),
            },
        }
    }
}

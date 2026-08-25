use crate::ids::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("data error: {0}")]
    Data(#[from] DataError),
    #[error("strategy error: {0}")]
    Strategy(#[from] StrategyError),
    #[error("search error: {0}")]
    Search(#[from] SearchError),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),
    #[error("validation error: {0}")]
    Validation(#[from] ValidationError),
    #[error("export error: {0}")]
    Export(#[from] ExportError),
    #[error("cancelled")]
    Cancelled,
    #[error("paused")]
    Paused,
    #[error("internal invariant violation: {message}")]
    Invariant { message: String },
}

macro_rules! simple_error {
    ($name:ident) => {
        #[derive(Debug, Error)]
        pub enum $name {
            #[error("{code}: {message}")]
            Message {
                code: String,
                message: String,
                retryable: bool,
            },
        }
    };
}

simple_error!(DataError);
simple_error!(StrategyError);
simple_error!(SearchError);
simple_error!(StorageError);
simple_error!(CheckpointError);
simple_error!(ValidationError);
simple_error!(ExportError);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ErrorCategory {
    Data,
    Strategy,
    Search,
    Storage,
    Checkpoint,
    Validation,
    Export,
    Cancellation,
    Internal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableError {
    pub code: String,
    pub category: ErrorCategory,
    pub message: String,
    pub component_id: Option<ComponentId>,
    pub strategy_id: Option<StrategyId>,
    pub parameter_set_id: Option<ParameterSetId>,
    pub retryable: bool,
    pub timestamp: i64,
}

impl PipelineError {
    pub fn to_serializable(&self, component_id: Option<ComponentId>) -> SerializableError {
        let now = chrono::Utc::now().timestamp_millis();
        SerializableError {
            code: match self {
                PipelineError::Cancelled => "CANCELLED".into(),
                PipelineError::Paused => "PAUSED".into(),
                _ => "PIPELINE_ERROR".into(),
            },
            category: match self {
                PipelineError::Data(_) => ErrorCategory::Data,
                PipelineError::Strategy(_) => ErrorCategory::Strategy,
                PipelineError::Search(_) => ErrorCategory::Search,
                PipelineError::Storage(_) => ErrorCategory::Storage,
                PipelineError::Checkpoint(_) => ErrorCategory::Checkpoint,
                PipelineError::Validation(_) => ErrorCategory::Validation,
                PipelineError::Export(_) => ErrorCategory::Export,
                PipelineError::Cancelled | PipelineError::Paused => ErrorCategory::Cancellation,
                PipelineError::Invariant { .. } => ErrorCategory::Internal,
            },
            message: self.to_string(),
            component_id,
            strategy_id: None,
            parameter_set_id: None,
            retryable: false,
            timestamp: now,
        }
    }
}

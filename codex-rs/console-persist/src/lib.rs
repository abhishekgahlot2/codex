pub mod checkpoint;
pub mod compaction;
pub mod export;
pub mod session;

// Re-export key types for convenience.
pub use checkpoint::{Checkpoint, CheckpointAction, CheckpointManager};
pub use compaction::{
    CompactionPolicy, CompactionResult, RollingSummary, messages_to_keep, should_compact,
};
pub use export::{ExportFormat, export_session, import_session};
pub use session::{
    DurableSession, JsonFileStore, PersistedMessage, PersistedTask, PersistedTeammate,
    SessionError, SessionStore,
};

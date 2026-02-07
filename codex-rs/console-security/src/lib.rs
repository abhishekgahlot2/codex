pub mod audit;
pub mod budget;
pub mod permission;
pub mod scope;

// Re-export key types for convenience.
pub use audit::{AuditEntry, AuditLog, RedactionPolicy};
pub use budget::{BudgetViolation, PerformanceBudget, ViolationSeverity};
pub use permission::{PermissionDecision, PermissionMode, PermissionPolicy, PermissionRule};
pub use scope::{CommandScope, FilesystemScope, ProviderScope};

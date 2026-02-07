pub mod capability;
pub mod hook;
pub mod lifecycle;
pub mod registry;

// Re-export key types for convenience.
pub use capability::{negotiate_capabilities, CapabilityGrant, PluginCapability, SandboxLevel};
pub use hook::{HookDecision, HookEvent, HookRegistry, HookResult, HookSpec};
pub use lifecycle::{LifecycleEvent, LifecycleTracker, PluginState};
pub use registry::{PluginMetadata, PluginRegistry};

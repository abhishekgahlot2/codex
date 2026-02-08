pub mod capability;
pub mod hook;
pub mod lifecycle;
pub mod registry;

// Re-export key types for convenience.
pub use capability::CapabilityGrant;
pub use capability::PluginCapability;
pub use capability::SandboxLevel;
pub use capability::negotiate_capabilities;
pub use hook::HookDecision;
pub use hook::HookEvent;
pub use hook::HookRegistry;
pub use hook::HookResult;
pub use hook::HookSpec;
pub use lifecycle::LifecycleEvent;
pub use lifecycle::LifecycleTracker;
pub use lifecycle::PluginState;
pub use registry::PluginMetadata;
pub use registry::PluginRegistry;

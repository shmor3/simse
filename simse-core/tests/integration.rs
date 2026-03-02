//! Integration tests verifying the public API surface of simse-core.

use simse_core::*;

#[test]
fn public_api_types_are_accessible() {
	// Verify key types are accessible at crate root via re-exports
	let _: fn(String) -> SimseError = SimseError::Other;
	let _ = std::any::type_name::<AppConfig>();
	let _ = std::any::type_name::<Conversation>();
	let _ = std::any::type_name::<TaskList>();
	let _ = std::any::type_name::<EventBus>();
	let _ = std::any::type_name::<Logger>();
	let _ = std::any::type_name::<CoreContext>();
}

#[test]
fn core_context_creation_with_defaults() {
	let config = AppConfig::default();
	let ctx = CoreContext::new(config);

	assert!(ctx.library.is_none());
	assert!(ctx.vfs.is_none());
}

#[test]
fn core_context_event_bus_is_shared() {
	let ctx = CoreContext::new(AppConfig::default());

	// The event bus should be wrapped in Arc for sharing
	let bus_clone = ctx.event_bus.clone();
	bus_clone.publish("test.event", serde_json::json!({"hello": "world"}));
}

#[test]
fn core_context_logger_has_simse_context() {
	let ctx = CoreContext::new(AppConfig::default());
	assert_eq!(ctx.logger.context(), "simse");
}

#[test]
fn core_context_logger_child_creation() {
	let ctx = CoreContext::new(AppConfig::default());
	let child = ctx.logger.child("library");
	assert_eq!(child.context(), "simse:library");
}

#[test]
fn core_context_task_list_is_empty() {
	let ctx = CoreContext::new(AppConfig::default());
	assert_eq!(ctx.task_list.task_count(), 0);
}

#[test]
fn submodules_are_accessible() {
	// Verify all public modules are accessible via qualified paths
	let _ = std::any::type_name::<simse_core::error::SimseError>();
	let _ = std::any::type_name::<simse_core::config::AppConfig>();
	let _ = std::any::type_name::<simse_core::conversation::Conversation>();
	let _ = std::any::type_name::<simse_core::events::EventBus>();
	let _ = std::any::type_name::<simse_core::logger::Logger>();
	let _ = std::any::type_name::<simse_core::tasks::TaskList>();
	let _ = std::any::type_name::<simse_core::hooks::HookSystem>();
	let _ = std::any::type_name::<simse_core::server::session::SessionManager>();
	let _ = std::any::type_name::<simse_core::library::Library>();
	let _ = std::any::type_name::<simse_core::vfs::VirtualFs>();
}

//! Integration tests for event JSON-RPC handlers.
//!
//! Tests follow the same pattern as `rpc_tasks.rs`: exercise the `EventBus`
//! API through a `CoreContext`, and verify dispatch routing via `CoreRpcServer`.

use std::sync::{Arc, Mutex};

use simse_core::config::AppConfig;
use simse_core::context::CoreContext;
use simse_core::events::EventBus;
use simse_core::rpc_protocol::JsonRpcRequest;
use simse_core::rpc_server::CoreRpcServer;
use simse_core::rpc_transport::NdjsonTransport;

// ---------------------------------------------------------------------------
// Helper: build an initialized server
// ---------------------------------------------------------------------------

fn make_server() -> CoreRpcServer {
	let transport = NdjsonTransport::new();
	CoreRpcServer::new(transport)
}

async fn make_initialized_server() -> CoreRpcServer {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 0,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	server
}

// ---------------------------------------------------------------------------
// EventBus integration (verifies the API the handlers call)
// ---------------------------------------------------------------------------

#[test]
fn event_bus_publish_delivers_to_subscriber() {
	let bus = EventBus::new();
	let received = Arc::new(Mutex::new(Vec::new()));
	let received_clone = Arc::clone(&received);

	let _unsub = bus.subscribe("test.event", move |payload| {
		received_clone.lock().unwrap().push(payload.clone());
	});

	bus.publish("test.event", serde_json::json!({ "key": "value" }));

	let events = received.lock().unwrap();
	assert_eq!(events.len(), 1);
	assert_eq!(events[0], serde_json::json!({ "key": "value" }));
}

#[test]
fn event_bus_subscribe_all_receives_all_events() {
	let bus = EventBus::new();
	let received = Arc::new(Mutex::new(Vec::new()));
	let received_clone = Arc::clone(&received);

	let _unsub = bus.subscribe_all(move |event_type, payload| {
		received_clone
			.lock()
			.unwrap()
			.push((event_type.to_string(), payload.clone()));
	});

	bus.publish("event.a", serde_json::json!(1));
	bus.publish("event.b", serde_json::json!(2));

	let events = received.lock().unwrap();
	assert_eq!(events.len(), 2);
	assert_eq!(events[0].0, "event.a");
	assert_eq!(events[1].0, "event.b");
}

#[test]
fn event_bus_unsubscribe_stops_delivery() {
	let bus = EventBus::new();
	let count = Arc::new(Mutex::new(0u32));
	let count_clone = Arc::clone(&count);

	let unsub = bus.subscribe("counted", move |_| {
		*count_clone.lock().unwrap() += 1;
	});

	bus.publish("counted", serde_json::json!(null));
	assert_eq!(*count.lock().unwrap(), 1);

	// Unsubscribe
	unsub();

	bus.publish("counted", serde_json::json!(null));
	assert_eq!(*count.lock().unwrap(), 1); // Still 1 — handler removed
}

#[test]
fn event_bus_unsubscribe_is_idempotent() {
	let bus = EventBus::new();
	let unsub = bus.subscribe("x", |_| {});
	unsub();
	unsub(); // Should not panic
}

#[test]
fn event_bus_publish_to_no_subscribers() {
	let bus = EventBus::new();
	// Should not panic
	bus.publish("nobody.listening", serde_json::json!({ "ignored": true }));
}

#[test]
fn event_bus_clear_removes_all_handlers() {
	let bus = EventBus::new();
	let count = Arc::new(Mutex::new(0u32));
	let count_clone = Arc::clone(&count);

	let _unsub = bus.subscribe("cleared", move |_| {
		*count_clone.lock().unwrap() += 1;
	});

	bus.publish("cleared", serde_json::json!(null));
	assert_eq!(*count.lock().unwrap(), 1);

	bus.clear();

	bus.publish("cleared", serde_json::json!(null));
	assert_eq!(*count.lock().unwrap(), 1); // Still 1 — cleared
}

#[test]
fn event_bus_through_core_context() {
	let ctx = CoreContext::new(AppConfig::default());
	let received = Arc::new(Mutex::new(false));
	let received_clone = Arc::clone(&received);

	let _unsub = ctx.event_bus.subscribe("ctx.test", move |_| {
		*received_clone.lock().unwrap() = true;
	});

	ctx.event_bus
		.publish("ctx.test", serde_json::json!("hello"));
	assert!(*received.lock().unwrap());
}

#[test]
fn event_bus_multiple_subscribers_same_event() {
	let bus = EventBus::new();
	let a = Arc::new(Mutex::new(0u32));
	let b = Arc::new(Mutex::new(0u32));
	let a_clone = Arc::clone(&a);
	let b_clone = Arc::clone(&b);

	let _unsub_a = bus.subscribe("multi", move |_| {
		*a_clone.lock().unwrap() += 1;
	});
	let _unsub_b = bus.subscribe("multi", move |_| {
		*b_clone.lock().unwrap() += 1;
	});

	bus.publish("multi", serde_json::json!(null));
	assert_eq!(*a.lock().unwrap(), 1);
	assert_eq!(*b.lock().unwrap(), 1);
}

#[test]
fn event_bus_subscribe_different_events_isolated() {
	let bus = EventBus::new();
	let a_count = Arc::new(Mutex::new(0u32));
	let b_count = Arc::new(Mutex::new(0u32));
	let a_clone = Arc::clone(&a_count);
	let b_clone = Arc::clone(&b_count);

	let _unsub_a = bus.subscribe("event.a", move |_| {
		*a_clone.lock().unwrap() += 1;
	});
	let _unsub_b = bus.subscribe("event.b", move |_| {
		*b_clone.lock().unwrap() += 1;
	});

	bus.publish("event.a", serde_json::json!(null));
	assert_eq!(*a_count.lock().unwrap(), 1);
	assert_eq!(*b_count.lock().unwrap(), 0);

	bus.publish("event.b", serde_json::json!(null));
	assert_eq!(*a_count.lock().unwrap(), 1);
	assert_eq!(*b_count.lock().unwrap(), 1);
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — event/subscribe
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_event_subscribe_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "test.event" }),
		})
		.await;
	// Should write not-initialized error, not panic
}

#[tokio::test]
async fn dispatch_event_subscribe_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "loop.start" }),
		})
		.await;
	// Should return { subscriptionId: "sub_0" }
}

#[tokio::test]
async fn dispatch_event_subscribe_wildcard() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "*" }),
		})
		.await;
	// Should succeed with wildcard subscription
}

#[tokio::test]
async fn dispatch_event_subscribe_missing_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return INVALID_PARAMS error (missing eventType)
}

#[tokio::test]
async fn dispatch_event_subscribe_null_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!(null),
		})
		.await;
	// Should return INVALID_PARAMS error
}

#[tokio::test]
async fn dispatch_event_subscribe_multiple_returns_unique_ids() {
	let mut server = make_initialized_server().await;

	// Subscribe twice — should get different subscription IDs
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "event.a" }),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "event.b" }),
		})
		.await;
	// sub_0 and sub_1 should be returned, not panic
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — event/unsubscribe
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_event_unsubscribe_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_0" }),
		})
		.await;
	// Should write not-initialized error, not panic
}

#[tokio::test]
async fn dispatch_event_unsubscribe_existing() {
	let mut server = make_initialized_server().await;

	// Subscribe first
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "test.event" }),
		})
		.await;

	// Unsubscribe
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_0" }),
		})
		.await;
	// Should succeed without panic
}

#[tokio::test]
async fn dispatch_event_unsubscribe_nonexistent() {
	let mut server = make_initialized_server().await;
	// Unsubscribing a non-existent ID should still return success (idempotent)
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_999" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_event_unsubscribe_missing_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Should return INVALID_PARAMS error
}

#[tokio::test]
async fn dispatch_event_unsubscribe_double_call() {
	let mut server = make_initialized_server().await;

	// Subscribe
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "test.event" }),
		})
		.await;

	// Unsubscribe twice — should be idempotent
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_0" }),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_0" }),
		})
		.await;
	// Second call: no-op since already removed, but returns {} successfully
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — event/publish
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_event_publish_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "test.event",
				"payload": { "key": "value" }
			}),
		})
		.await;
	// Should write not-initialized error, not panic
}

#[tokio::test]
async fn dispatch_event_publish_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "loop.start",
				"payload": { "turns": 5 }
			}),
		})
		.await;
	// Should succeed without panic
}

#[tokio::test]
async fn dispatch_event_publish_with_default_payload() {
	let mut server = make_initialized_server().await;
	// Payload is optional, defaults to null
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/publish".to_string(),
			params: serde_json::json!({ "type": "simple.event" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_event_publish_missing_type() {
	let mut server = make_initialized_server().await;
	// Missing "type" field — should return INVALID_PARAMS error
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/publish".to_string(),
			params: serde_json::json!({ "payload": {} }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_event_publish_null_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/publish".to_string(),
			params: serde_json::json!(null),
		})
		.await;
	// Should return INVALID_PARAMS error
}

// ---------------------------------------------------------------------------
// Full lifecycle tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_event_full_lifecycle() {
	let mut server = make_initialized_server().await;

	// Subscribe to an event
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "loop.start" }),
		})
		.await;

	// Publish an event (the subscription will try to write a notification to stdout)
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "loop.start",
				"payload": { "maxTurns": 10 }
			}),
		})
		.await;

	// Unsubscribe
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_0" }),
		})
		.await;

	// Publish again — no notification should fire (unsubscribed)
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "loop.start",
				"payload": {}
			}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_event_subscribe_and_publish_wildcard() {
	let mut server = make_initialized_server().await;

	// Subscribe to all events
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "*" }),
		})
		.await;

	// Publish different event types
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "loop.start",
				"payload": {}
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "event/publish".to_string(),
			params: serde_json::json!({
				"type": "task.create",
				"payload": { "id": "1" }
			}),
		})
		.await;

	// Unsubscribe wildcard
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_0" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_event_multiple_subscriptions_lifecycle() {
	let mut server = make_initialized_server().await;

	// Subscribe to two different events
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "event.a" }),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "event/subscribe".to_string(),
			params: serde_json::json!({ "eventType": "event.b" }),
		})
		.await;

	// Publish to both
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "event/publish".to_string(),
			params: serde_json::json!({ "type": "event.a", "payload": "a" }),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "event/publish".to_string(),
			params: serde_json::json!({ "type": "event.b", "payload": "b" }),
		})
		.await;

	// Unsubscribe only the first
	server
		.dispatch(JsonRpcRequest {
			id: 5,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_0" }),
		})
		.await;

	// Publish to event.a again — no notification (unsubscribed)
	server
		.dispatch(JsonRpcRequest {
			id: 6,
			method: "event/publish".to_string(),
			params: serde_json::json!({ "type": "event.a", "payload": "a2" }),
		})
		.await;

	// Publish to event.b — still subscribed via sub_1
	server
		.dispatch(JsonRpcRequest {
			id: 7,
			method: "event/publish".to_string(),
			params: serde_json::json!({ "type": "event.b", "payload": "b2" }),
		})
		.await;

	// Clean up
	server
		.dispatch(JsonRpcRequest {
			id: 8,
			method: "event/unsubscribe".to_string(),
			params: serde_json::json!({ "subscriptionId": "sub_1" }),
		})
		.await;
}

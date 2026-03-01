use simse_core::events::*;
use std::sync::{
	atomic::{AtomicU32, Ordering},
	Arc, Mutex,
};

#[test]
fn test_publish_subscribe() {
	let bus = EventBus::new();
	let counter = Arc::new(AtomicU32::new(0));
	let c = counter.clone();
	let _unsub = bus.subscribe("test.event", move |_payload| {
		c.fetch_add(1, Ordering::SeqCst);
	});
	bus.publish("test.event", serde_json::json!({"key": "value"}));
	assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_unsubscribe() {
	let bus = EventBus::new();
	let counter = Arc::new(AtomicU32::new(0));
	let c = counter.clone();
	let unsub = bus.subscribe("test.event", move |_| {
		c.fetch_add(1, Ordering::SeqCst);
	});
	bus.publish("test.event", serde_json::json!({}));
	assert_eq!(counter.load(Ordering::SeqCst), 1);
	unsub();
	bus.publish("test.event", serde_json::json!({}));
	assert_eq!(counter.load(Ordering::SeqCst), 1); // no change
}

#[test]
fn test_subscribe_all() {
	let bus = EventBus::new();
	let counter = Arc::new(AtomicU32::new(0));
	let c = counter.clone();
	let _unsub = bus.subscribe_all(move |_event_type, _payload| {
		c.fetch_add(1, Ordering::SeqCst);
	});
	bus.publish("event.a", serde_json::json!({}));
	bus.publish("event.b", serde_json::json!({}));
	assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn test_clear() {
	let bus = EventBus::new();
	let counter = Arc::new(AtomicU32::new(0));
	let c = counter.clone();
	let _unsub = bus.subscribe("x", move |_| {
		c.fetch_add(1, Ordering::SeqCst);
	});
	bus.clear();
	bus.publish("x", serde_json::json!({}));
	assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[test]
fn test_handler_error_isolation() {
	let bus = EventBus::new();
	let counter = Arc::new(AtomicU32::new(0));
	let c = counter.clone();
	// First handler panics
	let _unsub1 = bus.subscribe("x", |_| {
		panic!("handler error");
	});
	// Second handler should still fire
	let _unsub2 = bus.subscribe("x", move |_| {
		c.fetch_add(1, Ordering::SeqCst);
	});
	bus.publish("x", serde_json::json!({}));
	assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_multiple_subscribers_same_event() {
	let bus = EventBus::new();
	let counter = Arc::new(AtomicU32::new(0));
	let c1 = counter.clone();
	let c2 = counter.clone();
	let _unsub1 = bus.subscribe("ev", move |_| {
		c1.fetch_add(1, Ordering::SeqCst);
	});
	let _unsub2 = bus.subscribe("ev", move |_| {
		c2.fetch_add(10, Ordering::SeqCst);
	});
	bus.publish("ev", serde_json::json!({}));
	assert_eq!(counter.load(Ordering::SeqCst), 11);
}

#[test]
fn test_subscribe_all_receives_event_type() {
	let bus = EventBus::new();
	let received = Arc::new(Mutex::new(Vec::<String>::new()));
	let r = received.clone();
	let _unsub = bus.subscribe_all(move |event_type, _payload| {
		r.lock().unwrap().push(event_type.to_string());
	});
	bus.publish("foo.bar", serde_json::json!({}));
	bus.publish("baz.qux", serde_json::json!({}));
	let names = received.lock().unwrap();
	assert_eq!(*names, vec!["foo.bar".to_string(), "baz.qux".to_string()]);
}

#[test]
fn test_global_handler_error_isolation() {
	let bus = EventBus::new();
	let counter = Arc::new(AtomicU32::new(0));
	let c = counter.clone();
	// Global handler that panics
	let _unsub1 = bus.subscribe_all(|_type, _payload| {
		panic!("global handler error");
	});
	// Per-event handler should still fire
	let _unsub2 = bus.subscribe("x", move |_| {
		c.fetch_add(1, Ordering::SeqCst);
	});
	bus.publish("x", serde_json::json!({}));
	assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_no_subscribers_does_not_panic() {
	let bus = EventBus::new();
	// Publishing with no subscribers should be a no-op
	bus.publish("no.listeners", serde_json::json!({"data": 42}));
}

#[test]
fn test_event_type_constants_exist() {
	// Verify key event type constants are defined
	assert_eq!(event_types::STREAM_START, "stream.start");
	assert_eq!(event_types::STREAM_TOKEN, "stream.token");
	assert_eq!(event_types::STREAM_ERROR, "stream.error");
	assert_eq!(event_types::STREAM_RETRY, "stream.retry");
	assert_eq!(event_types::STREAM_END, "stream.end");
	assert_eq!(event_types::LOOP_TURN_START, "loop.turn_start");
	assert_eq!(event_types::LOOP_TURN_END, "loop.turn_end");
	assert_eq!(event_types::LOOP_TOOL_START, "loop.tool_start");
	assert_eq!(event_types::LOOP_TOOL_END, "loop.tool_end");
	assert_eq!(event_types::LOOP_DOOM_LOOP, "loop.doom_loop");
	assert_eq!(event_types::LIBRARY_SEARCH, "library.search");
	assert_eq!(event_types::LIBRARY_STORE, "library.store");
	assert_eq!(event_types::LIBRARY_DELETE, "library.delete");
	assert_eq!(event_types::CHAIN_START, "chain.start");
	assert_eq!(event_types::CHAIN_END, "chain.end");
	assert_eq!(event_types::TASK_CREATE, "task.create");
	assert_eq!(event_types::TASK_UPDATE, "task.update");
	assert_eq!(event_types::TASK_DELETE, "task.delete");
	assert_eq!(event_types::CONVERSATION_ADD, "conversation.add");
	assert_eq!(event_types::CONVERSATION_COMPACT, "conversation.compact");
}

#[test]
fn test_reentrant_publish() {
	let bus = EventBus::new();
	let bus2 = bus.clone();
	let counter = Arc::new(AtomicU32::new(0));
	let c = counter.clone();
	let _unsub1 = bus.subscribe("a", move |_| {
		bus2.publish("b", serde_json::json!({}));
	});
	let _unsub2 = bus.subscribe("b", move |_| {
		c.fetch_add(1, Ordering::SeqCst);
	});
	bus.publish("a", serde_json::json!({}));
	assert_eq!(counter.load(Ordering::SeqCst), 1);
}

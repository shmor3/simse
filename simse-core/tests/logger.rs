use simse_core::logger::*;

#[test]
fn test_logger_creation() {
    let logger = Logger::new("test");
    // Should not panic
    logger.info("hello");
}

#[test]
fn test_child_logger() {
    let parent = Logger::new("parent");
    let child = parent.child("child");
    // Child context should be "parent:child"
    child.info("from child");
}

#[test]
fn test_log_level_filtering() {
    let logger = Logger::new("test");
    logger.set_level(LogLevel::Warn);
    assert_eq!(logger.get_level(), LogLevel::Warn);
    // debug and info should be filtered (no-op)
    logger.debug("should not appear");
    logger.info("should not appear");
    // warn and error should pass through
    logger.warn("warning");
    logger.error("error");
}

#[test]
fn test_shared_level_between_parent_and_child() {
    let parent = Logger::new("parent");
    let child = parent.child("child");
    parent.set_level(LogLevel::Error);
    // Child should also be at Error level since they share state
    assert_eq!(child.get_level(), LogLevel::Error);
}

#[test]
fn test_noop_logger() {
    let logger = create_noop_logger();
    // All methods should be no-ops
    logger.debug("noop");
    logger.info("noop");
    logger.warn("noop");
    logger.error("noop");
    let child = logger.child("child");
    child.info("also noop");
}

#[test]
fn test_log_level_priority() {
    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Warn);
    assert!(LogLevel::Warn < LogLevel::Error);
    assert!(LogLevel::Error < LogLevel::None);
}

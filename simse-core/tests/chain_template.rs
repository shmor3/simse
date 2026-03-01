use simse_core::chain::template::*;

#[test]
fn test_create_template() {
	let t = PromptTemplate::new("Hello {name}!").unwrap();
	assert!(t.has_variables());
	assert_eq!(t.variables(), vec!["name"]);
}

#[test]
fn test_format() {
	let t = PromptTemplate::new("Hello {name}, welcome to {place}!").unwrap();
	let mut values = std::collections::HashMap::new();
	values.insert("name".into(), "Alice".into());
	values.insert("place".into(), "Rust".into());
	assert_eq!(t.format(&values).unwrap(), "Hello Alice, welcome to Rust!");
}

#[test]
fn test_missing_variable() {
	let t = PromptTemplate::new("Hello {name}!").unwrap();
	let values = std::collections::HashMap::new();
	assert!(t.format(&values).is_err());
}

#[test]
fn test_empty_template_rejected() {
	assert!(PromptTemplate::new("").is_err());
}

#[test]
fn test_no_variables() {
	let t = PromptTemplate::new("static text").unwrap();
	assert!(!t.has_variables());
	assert_eq!(t.format(&Default::default()).unwrap(), "static text");
}

#[test]
fn test_duplicate_variables_deduped() {
	let t = PromptTemplate::new("{a} and {a}").unwrap();
	assert_eq!(t.variables(), vec!["a"]);
}

#[test]
fn test_hyphenated_variables() {
	let t = PromptTemplate::new("{my-var}").unwrap();
	assert_eq!(t.variables(), vec!["my-var"]);
}

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::AdaptiveError;

/// Dimensionality of entry type one-hot encoding: fact, decision, observation.
const ENTRY_TYPE_DIM: usize = 3;
/// Dimensionality of temporal features: timestamp, time_since_last, session_ordinal.
const TEMPORAL_DIM: usize = 3;
/// Dimensionality of action one-hot encoding: extraction, compendium, reorganization, optimization.
const ACTION_DIM: usize = 4;
/// Sum of all fixed (non-vocabulary) feature dimensions.
const FIXED_DIM: usize = ENTRY_TYPE_DIM + TEMPORAL_DIM + ACTION_DIM; // 10

/// Serializable snapshot of the vocabulary state, used for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabularyState {
    pub topics: Vec<String>,
    pub tags: Vec<String>,
    pub max_topics: usize,
    pub max_tags: usize,
}

/// Maintains string-to-index mappings for topics and tags, and provides
/// encoding methods that convert raw library data into fixed-size feature vectors
/// suitable for input to the predictive coding network.
///
/// The full input vector layout is:
///   [ topic_one_hot | tag_bitmap | entry_type_one_hot | temporal_features | action_one_hot ]
///     max_topics      max_tags     3                    3                   4
pub struct VocabularyManager {
    topic_to_idx: HashMap<String, usize>,
    tag_to_idx: HashMap<String, usize>,
    topics: Vec<String>,
    tags: Vec<String>,
    max_topics: usize,
    max_tags: usize,
}

impl VocabularyManager {
    /// Create a new empty vocabulary with the given capacity limits.
    pub fn new(max_topics: usize, max_tags: usize) -> Self {
        Self {
            topic_to_idx: HashMap::new(),
            tag_to_idx: HashMap::new(),
            topics: Vec::new(),
            tags: Vec::new(),
            max_topics,
            max_tags,
        }
    }

    /// Restore a vocabulary from a serialized state.
    pub fn from_state(state: VocabularyState) -> Self {
        let mut topic_to_idx = HashMap::with_capacity(state.topics.len());
        for (idx, topic) in state.topics.iter().enumerate() {
            topic_to_idx.insert(topic.clone(), idx);
        }

        let mut tag_to_idx = HashMap::with_capacity(state.tags.len());
        for (idx, tag) in state.tags.iter().enumerate() {
            tag_to_idx.insert(tag.clone(), idx);
        }

        Self {
            topic_to_idx,
            tag_to_idx,
            topics: state.topics,
            tags: state.tags,
            max_topics: state.max_topics,
            max_tags: state.max_tags,
        }
    }

    /// Number of topics currently registered.
    pub fn topic_count(&self) -> usize {
        self.topics.len()
    }

    /// Number of tags currently registered.
    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }

    /// Total dimensionality of the encoded feature vector:
    /// max_topics (one-hot) + max_tags (bitmap) + FIXED_DIM (entry_type + temporal + action).
    pub fn total_dim(&self) -> usize {
        self.max_topics + self.max_tags + FIXED_DIM
    }

    /// Register a topic and return its index. Idempotent: returns the existing
    /// index if the topic is already registered.
    ///
    /// Returns `AdaptiveError::VocabularyOverflow` if the topic limit has been reached
    /// and the topic is not already registered.
    pub fn register_topic(&mut self, topic: &str) -> Result<usize, AdaptiveError> {
        if let Some(&idx) = self.topic_to_idx.get(topic) {
            return Ok(idx);
        }
        if self.topics.len() >= self.max_topics {
            return Err(AdaptiveError::VocabularyOverflow(format!(
                "topic limit {} reached",
                self.max_topics
            )));
        }
        let idx = self.topics.len();
        self.topics.push(topic.to_string());
        self.topic_to_idx.insert(topic.to_string(), idx);
        Ok(idx)
    }

    /// Register a tag and return its index. Idempotent: returns the existing
    /// index if the tag is already registered.
    ///
    /// Returns `AdaptiveError::VocabularyOverflow` if the tag limit has been reached
    /// and the tag is not already registered.
    pub fn register_tag(&mut self, tag: &str) -> Result<usize, AdaptiveError> {
        if let Some(&idx) = self.tag_to_idx.get(tag) {
            return Ok(idx);
        }
        if self.tags.len() >= self.max_tags {
            return Err(AdaptiveError::VocabularyOverflow(format!(
                "tag limit {} reached",
                self.max_tags
            )));
        }
        let idx = self.tags.len();
        self.tags.push(tag.to_string());
        self.tag_to_idx.insert(tag.to_string(), idx);
        Ok(idx)
    }

    /// Encode a topic as a one-hot vector of length `max_topics`.
    /// Returns all zeros if the topic is unknown.
    pub fn encode_topic(&self, topic: &str) -> Vec<f64> {
        let mut vec = vec![0.0; self.max_topics];
        if let Some(&idx) = self.topic_to_idx.get(topic) {
            vec[idx] = 1.0;
        }
        vec
    }

    /// Encode a set of tags as a bitmap vector of length `max_tags`.
    /// Unknown tags are silently ignored.
    pub fn encode_tags(&self, tags: &[String]) -> Vec<f64> {
        let mut vec = vec![0.0; self.max_tags];
        for tag in tags {
            if let Some(&idx) = self.tag_to_idx.get(tag.as_str()) {
                vec[idx] = 1.0;
            }
        }
        vec
    }

    /// Encode an entry type as a one-hot vector of length 3.
    /// Recognized types: "fact" (0), "decision" (1), "observation" (2).
    /// Unknown types produce all zeros.
    pub fn encode_entry_type(entry_type: &str) -> Vec<f64> {
        let mut vec = vec![0.0; ENTRY_TYPE_DIM];
        match entry_type {
            "fact" => vec[0] = 1.0,
            "decision" => vec[1] = 1.0,
            "observation" => vec[2] = 1.0,
            _ => {}
        }
        vec
    }

    /// Encode temporal features as a vector of length 3.
    ///
    /// * `timestamp` - absolute timestamp (e.g. epoch seconds), normalized by caller
    /// * `time_since_last` - seconds since the previous entry
    /// * `session_ordinal` - ordinal position within the session
    pub fn encode_temporal(timestamp: f64, time_since_last: f64, session_ordinal: f64) -> Vec<f64> {
        vec![timestamp, time_since_last, session_ordinal]
    }

    /// Encode an action as a one-hot vector of length 4.
    /// Recognized actions: "extraction" (0), "compendium" (1),
    /// "reorganization" (2), "optimization" (3).
    /// Unknown actions produce all zeros.
    pub fn encode_action(action: &str) -> Vec<f64> {
        let mut vec = vec![0.0; ACTION_DIM];
        match action {
            "extraction" => vec[0] = 1.0,
            "compendium" => vec[1] = 1.0,
            "reorganization" => vec[2] = 1.0,
            "optimization" => vec[3] = 1.0,
            _ => {}
        }
        vec
    }

    /// Serialize the vocabulary state for persistence.
    pub fn serialize(&self) -> VocabularyState {
        VocabularyState {
            topics: self.topics.clone(),
            tags: self.tags.clone(),
            max_topics: self.max_topics,
            max_tags: self.max_tags,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_vocabulary_is_empty() {
        let vm = VocabularyManager::new(100, 200);
        assert_eq!(vm.topic_count(), 0);
        assert_eq!(vm.tag_count(), 0);
        assert_eq!(vm.topics.len(), 0);
        assert_eq!(vm.tags.len(), 0);
    }

    #[test]
    fn register_topic_returns_index() {
        let mut vm = VocabularyManager::new(10, 10);

        let idx0 = vm.register_topic("rust").unwrap();
        assert_eq!(idx0, 0);

        let idx1 = vm.register_topic("python").unwrap();
        assert_eq!(idx1, 1);

        // Idempotent: re-registering returns the same index
        let idx0_again = vm.register_topic("rust").unwrap();
        assert_eq!(idx0_again, 0);

        assert_eq!(vm.topic_count(), 2);
    }

    #[test]
    fn register_tag_returns_index() {
        let mut vm = VocabularyManager::new(10, 10);

        let idx0 = vm.register_tag("important").unwrap();
        assert_eq!(idx0, 0);

        let idx1 = vm.register_tag("archived").unwrap();
        assert_eq!(idx1, 1);

        // Idempotent: re-registering returns the same index
        let idx0_again = vm.register_tag("important").unwrap();
        assert_eq!(idx0_again, 0);

        assert_eq!(vm.tag_count(), 2);
    }

    #[test]
    fn topic_overflow_returns_error() {
        let mut vm = VocabularyManager::new(2, 10);

        vm.register_topic("a").unwrap();
        vm.register_topic("b").unwrap();

        let result = vm.register_topic("c");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "PCN_VOCABULARY_OVERFLOW");
    }

    #[test]
    fn tag_overflow_returns_error() {
        let mut vm = VocabularyManager::new(10, 2);

        vm.register_tag("x").unwrap();
        vm.register_tag("y").unwrap();

        let result = vm.register_tag("z");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "PCN_VOCABULARY_OVERFLOW");
    }

    #[test]
    fn encode_topic_one_hot() {
        let mut vm = VocabularyManager::new(5, 5);
        vm.register_topic("alpha").unwrap();
        vm.register_topic("beta").unwrap();
        vm.register_topic("gamma").unwrap();

        let encoded = vm.encode_topic("beta");
        assert_eq!(encoded.len(), 5);
        assert_eq!(encoded[0], 0.0);
        assert_eq!(encoded[1], 1.0); // beta is at index 1
        assert_eq!(encoded[2], 0.0);
        assert_eq!(encoded[3], 0.0);
        assert_eq!(encoded[4], 0.0);
    }

    #[test]
    fn encode_unknown_topic_is_zeros() {
        let vm = VocabularyManager::new(5, 5);
        let encoded = vm.encode_topic("nonexistent");
        assert_eq!(encoded.len(), 5);
        assert!(encoded.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn encode_tags_bitmap() {
        let mut vm = VocabularyManager::new(5, 5);
        vm.register_tag("red").unwrap();
        vm.register_tag("green").unwrap();
        vm.register_tag("blue").unwrap();

        let tags = vec!["red".to_string(), "blue".to_string()];
        let encoded = vm.encode_tags(&tags);
        assert_eq!(encoded.len(), 5);
        assert_eq!(encoded[0], 1.0); // red
        assert_eq!(encoded[1], 0.0); // green (not selected)
        assert_eq!(encoded[2], 1.0); // blue
        assert_eq!(encoded[3], 0.0);
        assert_eq!(encoded[4], 0.0);
    }

    #[test]
    fn total_dim_accounts_for_all_structured_features() {
        let vm = VocabularyManager::new(500, 1000);
        // total = max_topics(500) + max_tags(1000) + FIXED_DIM(10)
        assert_eq!(vm.total_dim(), 500 + 1000 + 10);
        assert_eq!(vm.total_dim(), 1510);
    }

    #[test]
    fn serialize_restore_round_trip() {
        let mut vm = VocabularyManager::new(100, 200);
        vm.register_topic("rust").unwrap();
        vm.register_topic("python").unwrap();
        vm.register_tag("important").unwrap();
        vm.register_tag("archived").unwrap();
        vm.register_tag("pinned").unwrap();

        let state = vm.serialize();
        let json = serde_json::to_string(&state).unwrap();
        let restored_state: VocabularyState = serde_json::from_str(&json).unwrap();
        let vm2 = VocabularyManager::from_state(restored_state);

        assert_eq!(vm2.topic_count(), 2);
        assert_eq!(vm2.tag_count(), 3);
        assert_eq!(vm2.total_dim(), vm.total_dim());

        // Encoding should produce identical results
        assert_eq!(vm.encode_topic("rust"), vm2.encode_topic("rust"));
        assert_eq!(vm.encode_topic("python"), vm2.encode_topic("python"));
        assert_eq!(
            vm.encode_tags(&["important".to_string(), "pinned".to_string()]),
            vm2.encode_tags(&["important".to_string(), "pinned".to_string()])
        );
    }

    #[test]
    fn encode_entry_type_known_types() {
        let fact = VocabularyManager::encode_entry_type("fact");
        assert_eq!(fact, vec![1.0, 0.0, 0.0]);

        let decision = VocabularyManager::encode_entry_type("decision");
        assert_eq!(decision, vec![0.0, 1.0, 0.0]);

        let observation = VocabularyManager::encode_entry_type("observation");
        assert_eq!(observation, vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn encode_entry_type_unknown_is_zeros() {
        let unknown = VocabularyManager::encode_entry_type("unknown");
        assert_eq!(unknown, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn encode_temporal_passes_through() {
        let temporal = VocabularyManager::encode_temporal(1000.0, 60.0, 5.0);
        assert_eq!(temporal, vec![1000.0, 60.0, 5.0]);
    }

    #[test]
    fn encode_action_known_actions() {
        let extraction = VocabularyManager::encode_action("extraction");
        assert_eq!(extraction, vec![1.0, 0.0, 0.0, 0.0]);

        let compendium = VocabularyManager::encode_action("compendium");
        assert_eq!(compendium, vec![0.0, 1.0, 0.0, 0.0]);

        let reorg = VocabularyManager::encode_action("reorganization");
        assert_eq!(reorg, vec![0.0, 0.0, 1.0, 0.0]);

        let opt = VocabularyManager::encode_action("optimization");
        assert_eq!(opt, vec![0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn encode_action_unknown_is_zeros() {
        let unknown = VocabularyManager::encode_action("unknown");
        assert_eq!(unknown, vec![0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn encode_tags_ignores_unknown_tags() {
        let mut vm = VocabularyManager::new(5, 5);
        vm.register_tag("known").unwrap();

        let tags = vec!["known".to_string(), "unknown".to_string()];
        let encoded = vm.encode_tags(&tags);
        assert_eq!(encoded[0], 1.0);
        // All others zero (including unknown)
        assert!(encoded[1..].iter().all(|&v| v == 0.0));
    }
}

use serde::{Deserialize, Serialize};

use crate::error::AdaptiveError;
use crate::vocabulary::VocabularyManager;

/// A library event carrying an embedding vector and structured metadata,
/// ready to be encoded into a combined input vector for the PCN.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEvent {
    pub embedding: Vec<f32>,
    pub topic: String,
    pub tags: Vec<String>,
    pub entry_type: String,
    pub timestamp: f64,
    pub time_since_last: f64,
    pub session_ordinal: f64,
    pub action: String,
}

/// Encodes [`LibraryEvent`]s into combined input vectors for the predictive
/// coding network.
///
/// The output vector layout is:
///   `[ embedding_f64 | topic_one_hot | tag_bitmap | entry_type_one_hot | temporal | action_one_hot ]`
///     embedding_dim    max_topics      max_tags     3                    3          4
///
/// The vocabulary portion uses max-sized encoding so that the output dimension
/// is consistent regardless of how many topics/tags have been registered.
/// The `encode` method returns a boolean indicating whether the vocabulary
/// grew (new topics or tags were registered), which is useful for downstream
/// bookkeeping even though the vector length remains stable.
pub struct InputEncoder {
    embedding_dim: usize,
    vocab: VocabularyManager,
}

impl InputEncoder {
    /// Create a new encoder with the given embedding dimension and vocabulary
    /// capacity limits.
    pub fn new(embedding_dim: usize, max_topics: usize, max_tags: usize) -> Self {
        Self {
            embedding_dim,
            vocab: VocabularyManager::new(max_topics, max_tags),
        }
    }

    /// Create an encoder from an existing [`VocabularyManager`], useful when
    /// restoring from a persisted vocabulary state.
    pub fn from_vocab(embedding_dim: usize, vocab: VocabularyManager) -> Self {
        Self {
            embedding_dim,
            vocab,
        }
    }

    /// Total dimensionality of the combined input vector:
    /// `embedding_dim + vocab.total_dim()`.
    pub fn current_input_dim(&self) -> usize {
        self.embedding_dim + self.vocab.total_dim()
    }

    /// Immutable reference to the underlying vocabulary manager.
    pub fn vocab(&self) -> &VocabularyManager {
        &self.vocab
    }

    /// Mutable reference to the underlying vocabulary manager.
    pub fn vocab_mut(&mut self) -> &mut VocabularyManager {
        &mut self.vocab
    }

    /// Encode a [`LibraryEvent`] into a combined input vector.
    ///
    /// Returns `(vector, grew)` where `grew` is `true` if the vocabulary
    /// registered any new topics or tags during this call.
    ///
    /// # Steps
    /// 1. Record current vocabulary size.
    /// 2. Register the event's topic and tags (may grow the vocabulary).
    /// 3. Determine whether the vocabulary grew.
    /// 4. Build the combined vector:
    ///    `[embedding_f64, topic_one_hot, tag_bitmap, entry_type_one_hot, temporal, action_one_hot]`
    /// 5. Pad the embedding with zeros if shorter than `embedding_dim`.
    pub fn encode(&mut self, event: &LibraryEvent) -> Result<(Vec<f64>, bool), AdaptiveError> {
        // 1. Snapshot vocabulary sizes before registration.
        let topics_before = self.vocab.topic_count();
        let tags_before = self.vocab.tag_count();

        // 2. Register topic and tags (may grow vocabulary or return overflow error).
        self.vocab.register_topic(&event.topic)?;
        for tag in &event.tags {
            self.vocab.register_tag(tag)?;
        }

        // 3. Check if vocabulary grew.
        let grew =
            self.vocab.topic_count() > topics_before || self.vocab.tag_count() > tags_before;

        // 4. Build the combined vector.
        let total_dim = self.current_input_dim();
        let mut combined = Vec::with_capacity(total_dim);

        // 4a. Embedding (f32 -> f64), padded with zeros if shorter than embedding_dim.
        for i in 0..self.embedding_dim {
            if i < event.embedding.len() {
                combined.push(event.embedding[i] as f64);
            } else {
                combined.push(0.0);
            }
        }

        // 4b. Topic one-hot.
        combined.extend(self.vocab.encode_topic(&event.topic));

        // 4c. Tag bitmap.
        combined.extend(self.vocab.encode_tags(&event.tags));

        // 4d. Entry type one-hot.
        combined.extend(VocabularyManager::encode_entry_type(&event.entry_type));

        // 4e. Temporal features.
        combined.extend(VocabularyManager::encode_temporal(
            event.timestamp,
            event.time_since_last,
            event.session_ordinal,
        ));

        // 4f. Action one-hot.
        combined.extend(VocabularyManager::encode_action(&event.action));

        debug_assert_eq!(
            combined.len(),
            total_dim,
            "Encoded vector length mismatch: got {}, expected {}",
            combined.len(),
            total_dim,
        );

        Ok((combined, grew))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(embedding: Vec<f32>) -> LibraryEvent {
        LibraryEvent {
            embedding,
            topic: "rust".to_string(),
            tags: vec!["important".to_string(), "core".to_string()],
            entry_type: "fact".to_string(),
            timestamp: 1000.0,
            time_since_last: 60.0,
            session_ordinal: 5.0,
            action: "extraction".to_string(),
        }
    }

    #[test]
    fn encode_event_produces_correct_length() {
        let embedding_dim = 8;
        let max_topics = 10;
        let max_tags = 20;
        let mut encoder = InputEncoder::new(embedding_dim, max_topics, max_tags);

        let event = make_event(vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);
        let (vec, _grew) = encoder.encode(&event).unwrap();

        let expected_dim = embedding_dim + max_topics + max_tags + 3 + 3 + 4;
        assert_eq!(vec.len(), expected_dim);
        assert_eq!(vec.len(), encoder.current_input_dim());
    }

    #[test]
    fn encode_preserves_embedding_values() {
        let embedding_dim = 4;
        let mut encoder = InputEncoder::new(embedding_dim, 5, 5);

        let embedding = vec![0.25_f32, 0.5, 0.75, 1.0];
        let event = make_event(embedding.clone());
        let (vec, _) = encoder.encode(&event).unwrap();

        // The first `embedding_dim` elements should be the embedding cast to f64.
        for (i, &val) in embedding.iter().enumerate() {
            assert!(
                (vec[i] - val as f64).abs() < 1e-6,
                "Embedding value mismatch at index {}: expected {}, got {}",
                i,
                val as f64,
                vec[i]
            );
        }
    }

    #[test]
    fn input_dim_grows_with_vocabulary() {
        // Two encoders with different vocabulary capacities should have
        // different input dimensions for the same embedding size.
        let enc_small = InputEncoder::new(8, 10, 20);
        let enc_large = InputEncoder::new(8, 50, 100);

        assert!(
            enc_large.current_input_dim() > enc_small.current_input_dim(),
            "Larger vocabulary capacity should yield larger input dim"
        );

        // Verify exact values.
        // small: 8 + 10 + 20 + 10 = 48
        assert_eq!(enc_small.current_input_dim(), 8 + 10 + 20 + 10);
        // large: 8 + 50 + 100 + 10 = 168
        assert_eq!(enc_large.current_input_dim(), 8 + 50 + 100 + 10);
    }

    #[test]
    fn encode_reports_vocabulary_growth() {
        let mut encoder = InputEncoder::new(4, 10, 10);

        // First event introduces new topic and tags -> grew = true.
        let event1 = make_event(vec![1.0; 4]);
        let (_vec, grew1) = encoder.encode(&event1).unwrap();
        assert!(grew1, "First encode should report vocabulary growth");

        // Same event again -> no new registrations -> grew = false.
        let (_vec, grew2) = encoder.encode(&event1).unwrap();
        assert!(
            !grew2,
            "Repeated encode with same vocab should not report growth"
        );

        // New topic -> grew = true.
        let mut event2 = make_event(vec![1.0; 4]);
        event2.topic = "python".to_string();
        event2.tags = vec!["important".to_string()]; // already known
        let (_vec, grew3) = encoder.encode(&event2).unwrap();
        assert!(grew3, "New topic should report vocabulary growth");
    }

    #[test]
    fn encode_pads_short_embedding() {
        let embedding_dim = 8;
        let mut encoder = InputEncoder::new(embedding_dim, 5, 5);

        // Embedding shorter than embedding_dim.
        let event = make_event(vec![1.0, 2.0, 3.0]); // only 3 elements
        let (vec, _) = encoder.encode(&event).unwrap();

        // Should still produce the correct total length.
        assert_eq!(vec.len(), encoder.current_input_dim());

        // First 3 values are from the embedding, rest are zero-padded.
        assert!((vec[0] - 1.0).abs() < 1e-6);
        assert!((vec[1] - 2.0).abs() < 1e-6);
        assert!((vec[2] - 3.0).abs() < 1e-6);
        for i in 3..embedding_dim {
            assert!(
                vec[i].abs() < 1e-6,
                "Padded value at index {} should be 0.0, got {}",
                i,
                vec[i]
            );
        }
    }

    #[test]
    fn encode_structured_features_correct() {
        let embedding_dim = 2;
        let max_topics = 3;
        let max_tags = 4;
        let mut encoder = InputEncoder::new(embedding_dim, max_topics, max_tags);

        let event = LibraryEvent {
            embedding: vec![0.5, 0.9],
            topic: "rust".to_string(),
            tags: vec!["core".to_string()],
            entry_type: "decision".to_string(),
            timestamp: 100.0,
            time_since_last: 10.0,
            session_ordinal: 2.0,
            action: "compendium".to_string(),
        };

        let (vec, _) = encoder.encode(&event).unwrap();

        // Layout: [emb(2), topic(3), tags(4), entry_type(3), temporal(3), action(4)]
        let offset = embedding_dim; // 2

        // Topic one-hot: "rust" is index 0 -> [1, 0, 0]
        assert_eq!(vec[offset], 1.0);
        assert_eq!(vec[offset + 1], 0.0);
        assert_eq!(vec[offset + 2], 0.0);

        let offset = offset + max_topics; // 5

        // Tag bitmap: "core" is index 0 -> [1, 0, 0, 0]
        assert_eq!(vec[offset], 1.0);
        assert_eq!(vec[offset + 1], 0.0);
        assert_eq!(vec[offset + 2], 0.0);
        assert_eq!(vec[offset + 3], 0.0);

        let offset = offset + max_tags; // 9

        // Entry type: "decision" -> [0, 1, 0]
        assert_eq!(vec[offset], 0.0);
        assert_eq!(vec[offset + 1], 1.0);
        assert_eq!(vec[offset + 2], 0.0);

        let offset = offset + 3; // 12

        // Temporal: [100.0, 10.0, 2.0]
        assert!((vec[offset] - 100.0).abs() < 1e-6);
        assert!((vec[offset + 1] - 10.0).abs() < 1e-6);
        assert!((vec[offset + 2] - 2.0).abs() < 1e-6);

        let offset = offset + 3; // 15

        // Action: "compendium" -> [0, 1, 0, 0]
        assert_eq!(vec[offset], 0.0);
        assert_eq!(vec[offset + 1], 1.0);
        assert_eq!(vec[offset + 2], 0.0);
        assert_eq!(vec[offset + 3], 0.0);
    }

    #[test]
    fn from_vocab_preserves_state() {
        let mut vocab = VocabularyManager::new(10, 10);
        vocab.register_topic("existing").unwrap();
        vocab.register_tag("old_tag").unwrap();

        let encoder = InputEncoder::from_vocab(8, vocab);

        assert_eq!(encoder.vocab().topic_count(), 1);
        assert_eq!(encoder.vocab().tag_count(), 1);
        assert_eq!(encoder.current_input_dim(), 8 + 10 + 10 + 10);
    }

    #[test]
    fn vocabulary_overflow_propagates() {
        let mut encoder = InputEncoder::new(4, 1, 1);

        // First event fills the vocab.
        let event1 = LibraryEvent {
            embedding: vec![1.0; 4],
            topic: "only_topic".to_string(),
            tags: vec!["only_tag".to_string()],
            entry_type: "fact".to_string(),
            timestamp: 0.0,
            time_since_last: 0.0,
            session_ordinal: 0.0,
            action: "extraction".to_string(),
        };
        encoder.encode(&event1).unwrap();

        // Second event with a new topic should overflow.
        let event2 = LibraryEvent {
            embedding: vec![1.0; 4],
            topic: "second_topic".to_string(),
            tags: vec!["only_tag".to_string()],
            entry_type: "fact".to_string(),
            timestamp: 0.0,
            time_since_last: 0.0,
            session_ordinal: 0.0,
            action: "extraction".to_string(),
        };
        let result = encoder.encode(&event2);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), "PCN_VOCABULARY_OVERFLOW");
    }
}

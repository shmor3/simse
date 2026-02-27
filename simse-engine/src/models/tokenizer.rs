use anyhow::Result;
use tokenizers::Tokenizer;

/// Incremental token decoder for streaming output.
///
/// Handles multi-byte sequences that span token boundaries by buffering
/// and only emitting valid UTF-8 text deltas.
pub struct TokenOutputStream {
    tokenizer: Tokenizer,
    tokens: Vec<u32>,
    prev_index: usize,
    current_index: usize,
}

impl TokenOutputStream {
    pub fn new(tokenizer: Tokenizer) -> Self {
        Self {
            tokenizer,
            tokens: vec![],
            prev_index: 0,
            current_index: 0,
        }
    }

    /// Process the next token and return the decoded text delta (if any).
    ///
    /// Returns `None` if the token is part of a multi-byte sequence
    /// that isn't complete yet.
    pub fn next_token(&mut self, token: u32) -> Result<Option<String>> {
        let prev_text = if self.tokens.is_empty() {
            String::new()
        } else {
            let prev_tokens = &self.tokens[self.prev_index..self.current_index];
            self.tokenizer
                .decode(prev_tokens, true)
                .map_err(|e| anyhow::anyhow!("Tokenizer decode error: {}", e))?
        };

        self.tokens.push(token);
        let current_text = {
            let current_tokens = &self.tokens[self.prev_index..];
            self.tokenizer
                .decode(current_tokens, true)
                .map_err(|e| anyhow::anyhow!("Tokenizer decode error: {}", e))?
        };
        self.current_index = self.tokens.len();

        if current_text.len() > prev_text.len() && current_text.starts_with(&prev_text) {
            let delta = current_text[prev_text.len()..].to_string();
            Ok(Some(delta))
        } else if current_text.len() > prev_text.len() {
            // Text changed in a non-trivial way — emit the full new text
            self.prev_index = self.current_index - 1;
            Ok(Some(current_text))
        } else {
            // No new text yet (buffering multi-byte)
            Ok(None)
        }
    }

    /// Decode any remaining bytes at the end of generation.
    pub fn decode_rest(&self) -> Result<Option<String>> {
        let prev_text = if self.prev_index < self.current_index {
            let prev_tokens = &self.tokens[self.prev_index..self.current_index];
            self.tokenizer
                .decode(prev_tokens, true)
                .map_err(|e| anyhow::anyhow!("Tokenizer decode error: {}", e))?
        } else {
            String::new()
        };

        let full_text = self
            .tokenizer
            .decode(&self.tokens[self.prev_index..], true)
            .map_err(|e| anyhow::anyhow!("Tokenizer decode error: {}", e))?;

        if full_text.len() > prev_text.len() {
            Ok(Some(full_text[prev_text.len()..].to_string()))
        } else {
            Ok(None)
        }
    }

    /// Get the underlying tokenizer reference.
    pub fn tokenizer(&self) -> &Tokenizer {
        &self.tokenizer
    }
}

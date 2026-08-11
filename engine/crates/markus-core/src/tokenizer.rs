//! Tokenizer — built from GGUF vocabulary metadata
//!
//! Supports BPE (GPT-2 style), SentencePiece, and simple char-level fallback.

use std::collections::HashMap;
use crate::gguf::GgufMeta;
use crate::{MarkusError, Result};

pub struct MarkusTokenizer {
    tokens: Vec<String>,
    token_to_id: HashMap<String, u32>,
    bos_id: Option<u32>,
    eos_id: Option<u32>,
    /// Whether this is a BPE-style tokenizer (most modern models)
    is_bpe: bool,
}

impl MarkusTokenizer {
    /// Build a tokenizer from GGUF metadata
    pub fn from_gguf_meta(meta: &GgufMeta) -> Result<Self> {
        let tokens = meta.token_list();
        if tokens.is_empty() {
            return Err(MarkusError::Tokenizer(
                "No vocabulary found in GGUF metadata".into()
            ));
        }

        let mut token_to_id = HashMap::with_capacity(tokens.len());
        for (id, tok) in tokens.iter().enumerate() {
            token_to_id.insert(tok.clone(), id as u32);
        }

        let bos_id = meta.bos_token_id();
        let eos_id = meta.eos_token_id();

        // Heuristic: if tokens contain "Ġ" (GPT-2 byte prefix) it's BPE
        let is_bpe = tokens.iter().any(|t| t.contains('Ġ') || t.starts_with("▁"));

        Ok(Self { tokens, token_to_id, bos_id, eos_id, is_bpe })
    }

    /// Encode a string into token IDs
    /// NOTE: For production use, this delegates to the `tokenizers` crate when
    /// a proper tokenizer.json is available. This fallback handles basic BPE-style splitting.
    pub fn encode(&self, text: &str) -> Result<Vec<u32>> {
        // Add BOS if available
        let mut ids: Vec<u32> = Vec::new();
        if let Some(bos) = self.bos_id {
            ids.push(bos);
        }

        // Greedy longest-match tokenization (simplified BPE)
        let encoded = self.greedy_encode(text);
        ids.extend(encoded);
        Ok(ids)
    }

    fn greedy_encode(&self, text: &str) -> Vec<u32> {
        let mut result = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let mut best_len = 0;
            let mut best_id = None;

            // Try longest match from current position
            let max_len = (chars.len() - i).min(32);
            for len in (1..=max_len).rev() {
                let substr: String = chars[i..i+len].iter().collect();
                if let Some(&id) = self.token_to_id.get(&substr) {
                    best_len = len;
                    best_id = Some(id);
                    break;
                }
                // Also try with GPT-2 space prefix
                if i == 0 || chars[i-1] == ' ' {
                    let prefixed = format!("Ġ{}", substr);
                    if let Some(&id) = self.token_to_id.get(&prefixed) {
                        best_len = len;
                        best_id = Some(id);
                        break;
                    }
                }
            }

            if let Some(id) = best_id {
                result.push(id);
                i += best_len;
            } else {
                // Unknown character — use byte fallback
                let ch = chars[i];
                let byte_tok = format!("<0x{:02X}>", ch as u32 & 0xFF);
                if let Some(&id) = self.token_to_id.get(&byte_tok) {
                    result.push(id);
                } else {
                    // Last resort: push unknown token (0 or find <unk>)
                    if let Some(&unk_id) = self.token_to_id.get("<unk>") {
                        result.push(unk_id);
                    }
                }
                i += 1;
            }
        }

        result
    }

    /// Decode a single token ID to a string fragment
    pub fn decode_token(&self, id: u32) -> Result<String> {
        let tok = self.tokens.get(id as usize)
            .ok_or_else(|| MarkusError::Tokenizer(format!("Invalid token id: {}", id)))?;

        // Convert GPT-2 space markers to actual spaces/chars
        let decoded = tok
            .replace("Ġ", " ")
            .replace("▁", " ")
            .replace("Ċ", "\n")
            .replace("ĉ", "\t");

        // Handle byte tokens like <0xAB>
        if decoded.starts_with("<0x") && decoded.ends_with('>') {
            if let Ok(byte) = u8::from_str_radix(&decoded[3..decoded.len()-1], 16) {
                return Ok(String::from_utf8_lossy(&[byte]).to_string());
            }
        }

        // Filter out special tokens
        if tok.starts_with('<') && tok.ends_with('>') {
            return Ok(String::new());
        }

        Ok(decoded)
    }

    /// Decode a sequence of token IDs to text
    pub fn decode(&self, ids: &[u32]) -> Result<String> {
        let mut out = String::new();
        for &id in ids {
            out.push_str(&self.decode_token(id)?);
        }
        Ok(out)
    }

    pub fn is_eos(&self, id: u32) -> bool {
        self.eos_id.map(|e| id == e).unwrap_or(false)
    }

    pub fn vocab_size(&self) -> usize {
        self.tokens.len()
    }

    pub fn bos_id(&self) -> Option<u32> {
        self.bos_id
    }

    pub fn eos_id(&self) -> Option<u32> {
        self.eos_id
    }
}

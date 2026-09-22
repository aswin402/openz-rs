use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub struct ClipTokenizer {
    vocab: HashMap<String, u32>,
    #[allow(dead_code)]
    merges: Vec<(String, String)>,
}

impl ClipTokenizer {
    pub fn load(model_dir: &Path) -> Self {
        // Try loading tokenizer.json
        let tokenizer_path = model_dir.join("tokenizer.json");
        if tokenizer_path.exists() {
            if let Ok(mut file) = File::open(&tokenizer_path) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    if let Ok(json) = serde_json::from_str::<Value>(&content) {
                        let mut vocab = HashMap::new();
                        let mut merges = Vec::new();

                        // Try extracting vocabulary
                        if let Some(vocab_obj) =
                            json.pointer("/model/vocab").and_then(|v| v.as_object())
                        {
                            for (k, v) in vocab_obj {
                                if let Some(id) = v.as_u64() {
                                    vocab.insert(k.clone(), id as u32);
                                }
                            }
                        }

                        // Try extracting BPE merges
                        if let Some(merges_arr) =
                            json.pointer("/model/merges").and_then(|v| v.as_array())
                        {
                            for m in merges_arr {
                                if let Some(m_str) = m.as_str() {
                                    let parts: Vec<&str> = m_str.split_whitespace().collect();
                                    if parts.len() == 2 {
                                        merges.push((parts[0].to_string(), parts[1].to_string()));
                                    }
                                }
                            }
                        }

                        if !vocab.is_empty() {
                            return Self { vocab, merges };
                        }
                    }
                }
            }
        }

        // Try loading vocab.json (older format)
        let vocab_path = model_dir.join("vocab.json");
        if vocab_path.exists() {
            if let Ok(mut file) = File::open(&vocab_path) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    if let Ok(vocab_map) = serde_json::from_str::<HashMap<String, u32>>(&content) {
                        return Self {
                            vocab: vocab_map,
                            merges: Vec::new(),
                        };
                    }
                }
            }
        }

        // Fallback: Create a minimal mock vocabulary for testing/safety
        let mut vocab = HashMap::new();
        vocab.insert("<|startoftext|>".to_string(), 49406);
        vocab.insert("<|endoftext|>".to_string(), 49407);
        Self {
            vocab,
            merges: Vec::new(),
        }
    }

    pub fn encode(&self, text: &str, max_length: usize) -> Vec<i32> {
        let mut tokens = Vec::new();

        // CLIP start of text token ID is typically 49406
        let sot_id = *self.vocab.get("<|startoftext|>").unwrap_or(&49406) as i32;
        let eot_id = *self.vocab.get("<|endoftext|>").unwrap_or(&49407) as i32;

        tokens.push(sot_id);

        let clean_text = text.to_lowercase();
        // Simple token split
        for word in clean_text.split_whitespace() {
            // Check if word is directly in vocab
            if let Some(&id) = self.vocab.get(word) {
                tokens.push(id as i32);
            } else {
                // If not, break it down or use character subwords, or standard unknown
                // For simplicity in fallback/mocking, just check substrings
                let mut found = false;
                for i in (1..=word.len()).rev() {
                    let sub = &word[0..i];
                    if let Some(&id) = self.vocab.get(sub) {
                        tokens.push(id as i32);
                        if i < word.len() {
                            let rest = &word[i..];
                            if let Some(&rest_id) = self.vocab.get(rest) {
                                tokens.push(rest_id as i32);
                            }
                        }
                        found = true;
                        break;
                    }
                }
                if !found {
                    // Map to a dummy ID if word isn't in vocab
                    tokens.push(0);
                }
            }
        }

        tokens.push(eot_id);

        // Pad or truncate
        if tokens.len() > max_length {
            tokens.truncate(max_length);
            tokens[max_length - 1] = eot_id;
        } else {
            while tokens.len() < max_length {
                tokens.push(0); // Pad with 0
            }
        }

        tokens
    }
}

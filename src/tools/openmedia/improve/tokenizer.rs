use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub struct ClipTokenizer {
    vocab: HashMap<String, u32>,
    #[allow(dead_code)]
    merges: Vec<(String, String)>,
}

impl ClipTokenizer {
    fn try_load_tokenizer_json(path: &Path) -> Option<Self> {
        let content = path.exists().then(|| std::fs::read_to_string(path).ok()).flatten()?;
        let json: Value = serde_json::from_str(&content).ok()?;
        let vocab: HashMap<String, u32> = json
            .pointer("/model/vocab")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_u64().map(|id| (k.clone(), id as u32)))
                    .collect()
            })
            .unwrap_or_default();
        (!vocab.is_empty()).then(|| {
            let merges = json
                .pointer("/model/merges")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.as_str())
                        .filter_map(|m_str| {
                            let parts: Vec<&str> = m_str.split_whitespace().collect();
                            (parts.len() == 2)
                                .then(|| (parts[0].to_string(), parts[1].to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            Self { vocab, merges }
        })
    }

    fn try_load_vocab_json(path: &Path) -> Option<Self> {
        let content = path.exists().then(|| std::fs::read_to_string(path).ok()).flatten()?;
        let vocab: HashMap<String, u32> = serde_json::from_str(&content).ok()?;
        Some(Self {
            vocab,
            merges: Vec::new(),
        })
    }

    pub fn load(model_dir: &Path) -> Self {
        Self::try_load_tokenizer_json(&model_dir.join("tokenizer.json"))
            .or_else(|| Self::try_load_vocab_json(&model_dir.join("vocab.json")))
            .unwrap_or_else(|| {
                let mut vocab = HashMap::new();
                vocab.insert("<|startoftext|>".to_string(), 49406);
                vocab.insert("<|endoftext|>".to_string(), 49407);
                Self {
                    vocab,
                    merges: Vec::new(),
                }
            })
    }

    pub fn encode(&self, text: &str, max_length: usize) -> Vec<i32> {
        let mut tokens = Vec::new();

        // CLIP start of text token ID is typically 49406
        let sot_id = *self.vocab.get("<|startoftext|>").unwrap_or(&49406) as i32;
        let eot_id = *self.vocab.get("<|endoftext|>").unwrap_or(&49407) as i32;

        tokens.push(sot_id);

        let clean_text = text.to_lowercase();
        for word in clean_text.split_whitespace() {
            match self.vocab.get(word).copied() {
                Some(id) => tokens.push(id as i32),
                None => {
                    let sub_match = (1..=word.len()).rev().find_map(|i| {
                        let sub = &word[0..i];
                        self.vocab.get(sub).copied().map(|id| (i, id))
                    });
                    match sub_match {
                        Some((i, id)) => {
                            tokens.push(id as i32);
                            let rest_id = (i < word.len())
                                .then(|| self.vocab.get(&word[i..]).copied())
                                .flatten();
                            match rest_id {
                                Some(rid) => tokens.push(rid as i32),
                                None => {}
                            }
                        }
                        None => tokens.push(0),
                    }
                }
            }
        }

        tokens.push(eot_id);

        // Pad or truncate
        match tokens.len() > max_length {
            true => {
                tokens.truncate(max_length);
                tokens[max_length - 1] = eot_id;
            }
            false => {
                while tokens.len() < max_length {
                    tokens.push(0);
                }
            }
        }

        tokens
    }
}

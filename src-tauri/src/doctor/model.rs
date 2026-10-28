//! Minimal Model2Vec (static embedding) runtime used by the Log Doctor.
//!
//! A Model2Vec model is a token-embedding table distilled from a sentence
//! transformer. A sentence embedding is the normalized mean of its WordPiece
//! token vectors, so inference is a tokenizer plus a table lookup: no ML
//! framework, GPU, or ONNX runtime is needed and a log line embeds in
//! microseconds.
//!
//! Supported files (the layout published by `minishlab/potion-*` models):
//! - `tokenizer.json`: Hugging Face tokenizer with a `WordPiece` model and a
//!   `BertNormalizer`/`BertPreTokenizer` pipeline.
//! - `model.safetensors`: a single F32 tensor shaped `[vocab_size, dim]`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct StaticModel {
    vocab: HashMap<String, u32>,
    unk_id: Option<u32>,
    max_chars_per_word: usize,
    lowercase: bool,
    dim: usize,
    /// Row-major `[vocab_size, dim]` embedding table.
    embeddings: Vec<f32>,
}

impl StaticModel {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let tokenizer_path = dir.join("tokenizer.json");
        let tokenizer: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&tokenizer_path)
                .map_err(|e| format!("Failed to read {}: {}", tokenizer_path.display(), e))?,
        )
        .map_err(|e| format!("Invalid tokenizer.json: {}", e))?;

        let model = &tokenizer["model"];
        if model["type"].as_str() != Some("WordPiece") {
            return Err("Unsupported tokenizer: only WordPiece models are supported".to_string());
        }

        let vocab: HashMap<String, u32> = model["vocab"]
            .as_object()
            .ok_or("tokenizer.json has no WordPiece vocabulary")?
            .iter()
            .filter_map(|(token, id)| Some((token.clone(), u32::try_from(id.as_u64()?).ok()?)))
            .collect();
        let unk_id = model["unk_token"]
            .as_str()
            .and_then(|token| vocab.get(token).copied());
        let max_chars_per_word = model["max_input_chars_per_word"]
            .as_u64()
            .map(|value| value as usize)
            .unwrap_or(100);
        let lowercase = tokenizer["normalizer"]["lowercase"]
            .as_bool()
            .unwrap_or(true);

        let (rows, dim, embeddings) = read_safetensors_matrix(&dir.join("model.safetensors"))?;
        let max_id = vocab.values().copied().max().unwrap_or(0) as usize;
        if rows <= max_id {
            return Err(format!(
                "Embedding table has {} rows but the vocabulary needs {}",
                rows,
                max_id + 1
            ));
        }

        Ok(Self {
            vocab,
            unk_id,
            max_chars_per_word,
            lowercase,
            dim,
            embeddings,
        })
    }

    /// Embed `text` as an L2-normalized vector, or `None` when no known token remains.
    pub fn embed(&self, text: &str) -> Option<Vec<f32>> {
        let ids = self.tokenize(text);
        if ids.is_empty() {
            return None;
        }

        let mut sum = vec![0.0_f32; self.dim];
        for id in &ids {
            let start = *id as usize * self.dim;
            for (acc, value) in sum
                .iter_mut()
                .zip(&self.embeddings[start..start + self.dim])
            {
                *acc += value;
            }
        }

        let norm = sum.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm <= f32::EPSILON {
            return None;
        }
        sum.iter_mut().for_each(|value| *value /= norm);
        Some(sum)
    }

    /// WordPiece token ids for `text`, without special tokens and with `[UNK]`
    /// removed (matching Model2Vec's own encoder).
    fn tokenize(&self, text: &str) -> Vec<u32> {
        let normalized = normalize(text, self.lowercase);
        let mut ids = Vec::new();
        for word in pre_tokenize(&normalized) {
            self.word_piece(word, &mut ids);
        }
        if let Some(unk) = self.unk_id {
            ids.retain(|id| *id != unk);
        }
        ids
    }

    fn word_piece(&self, word: &str, ids: &mut Vec<u32>) {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() > self.max_chars_per_word {
            ids.extend(self.unk_id);
            return;
        }

        let mut pieces = Vec::new();
        let mut start = 0;
        while start < chars.len() {
            let mut end = chars.len();
            let mut found = None;
            while start < end {
                let piece: String = chars[start..end].iter().collect();
                let candidate = if start > 0 {
                    format!("##{piece}")
                } else {
                    piece
                };
                if let Some(id) = self.vocab.get(&candidate) {
                    found = Some(*id);
                    break;
                }
                end -= 1;
            }

            match found {
                Some(id) => {
                    pieces.push(id);
                    start = end;
                }
                None => {
                    // WordPiece maps the whole word to [UNK] when any part is unknown.
                    ids.extend(self.unk_id);
                    return;
                }
            }
        }
        ids.extend(pieces);
    }
}

/// Cosine similarity of two L2-normalized vectors.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// BertNormalizer: drop control characters, map whitespace to spaces, pad CJK
/// ideographs, and lowercase. Accent stripping is skipped because service logs
/// are ASCII; accented words simply fall back to sub-word pieces.
fn normalize(text: &str, lowercase: bool) -> String {
    let mut output = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\0' || ch == '\u{fffd}' || (ch.is_control() && !ch.is_whitespace()) {
            continue;
        }
        if ch.is_whitespace() {
            output.push(' ');
        } else if is_cjk(ch) {
            output.push(' ');
            output.push(ch);
            output.push(' ');
        } else if lowercase {
            output.extend(ch.to_lowercase());
        } else {
            output.push(ch);
        }
    }
    output
}

/// BertPreTokenizer: split on whitespace, and make every punctuation
/// character its own token.
fn pre_tokenize(text: &str) -> Vec<&str> {
    let mut words = Vec::new();
    for chunk in text.split_whitespace() {
        let mut start = 0;
        for (index, ch) in chunk.char_indices() {
            if is_punctuation(ch) {
                if start < index {
                    words.push(&chunk[start..index]);
                }
                words.push(&chunk[index..index + ch.len_utf8()]);
                start = index + ch.len_utf8();
            }
        }
        if start < chunk.len() {
            words.push(&chunk[start..]);
        }
    }
    words
}

fn is_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation() || (!ch.is_ascii() && !ch.is_alphanumeric() && !is_cjk(ch))
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F
    )
}

/// Read the first F32 matrix from a `.safetensors` file.
///
/// Format: 8-byte little-endian header length, a JSON header describing each
/// tensor's dtype/shape/byte range, then the raw tensor bytes.
fn read_safetensors_matrix(path: &Path) -> Result<(usize, usize, Vec<f32>), String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let header_len = bytes
        .get(..8)
        .map(|raw| u64::from_le_bytes(raw.try_into().unwrap_or_default()) as usize)
        .ok_or("model.safetensors is truncated")?;
    let header_end = 8usize
        .checked_add(header_len)
        .filter(|end| *end <= bytes.len())
        .ok_or("model.safetensors header is truncated")?;
    let header: serde_json::Value = serde_json::from_slice(&bytes[8..header_end])
        .map_err(|e| format!("Invalid safetensors header: {}", e))?;

    let tensors = header.as_object().ok_or("Invalid safetensors header")?;
    let tensor = tensors
        .get("embeddings")
        .or_else(|| {
            tensors
                .iter()
                .find(|(name, _)| name.as_str() != "__metadata__")
                .map(|(_, tensor)| tensor)
        })
        .ok_or("model.safetensors contains no tensor")?;

    if tensor["dtype"].as_str() != Some("F32") {
        return Err("Only F32 embedding tables are supported".to_string());
    }
    let shape: Vec<usize> = tensor["shape"]
        .as_array()
        .map(|dims| {
            dims.iter()
                .filter_map(|dim| dim.as_u64())
                .map(|dim| dim as usize)
                .collect()
        })
        .unwrap_or_default();
    let [rows, dim] = shape[..] else {
        return Err("Embedding tensor must be two-dimensional".to_string());
    };
    let offsets: Vec<usize> = tensor["data_offsets"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_u64())
                .map(|value| value as usize)
                .collect()
        })
        .unwrap_or_default();
    let [start, end] = offsets[..] else {
        return Err("Embedding tensor has no data offsets".to_string());
    };

    let data = bytes
        .get(header_end + start..header_end + end)
        .ok_or("Embedding tensor data is truncated")?;
    if data.len() != rows * dim * 4 {
        return Err("Embedding tensor size does not match its shape".to_string());
    }

    let values = data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|raw| f32::from_le_bytes(*raw))
        .collect();
    Ok((rows, dim, values))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny model on disk: 6 tokens with 2-D embeddings.
    fn write_tiny_model(dir: &Path) {
        let tokenizer = serde_json::json!({
            "normalizer": { "type": "BertNormalizer", "lowercase": true },
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "max_input_chars_per_word": 100,
                "vocab": { "[UNK]": 0, "port": 1, "in": 2, "use": 3, "disk": 4, "##s": 5 }
            }
        });
        fs::write(dir.join("tokenizer.json"), tokenizer.to_string()).unwrap();

        let rows: [[f32; 2]; 6] = [
            [0.0, 0.0],
            [1.0, 0.0],
            [0.9, 0.1],
            [0.8, 0.2],
            [0.0, 1.0],
            [0.0, 0.5],
        ];
        let data: Vec<u8> = rows
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let header = serde_json::json!({
            "embeddings": { "dtype": "F32", "shape": [6, 2], "data_offsets": [0, data.len()] }
        })
        .to_string();
        let mut file = (header.len() as u64).to_le_bytes().to_vec();
        file.extend(header.as_bytes());
        file.extend(data);
        fs::write(dir.join("model.safetensors"), file).unwrap();
    }

    #[test]
    fn pre_tokenize_splits_punctuation() {
        assert_eq!(
            pre_tokenize("bind: address-in use"),
            vec!["bind", ":", "address", "-", "in", "use"]
        );
    }

    #[test]
    fn tokenizer_uses_word_pieces_and_drops_unknown_words() {
        let temp = tempfile::tempdir().unwrap();
        write_tiny_model(temp.path());
        let model = StaticModel::load(temp.path()).unwrap();

        assert_eq!(model.tokenize("PORT in use"), vec![1, 2, 3]);
        assert_eq!(model.tokenize("ports"), vec![1, 5]);
        assert_eq!(model.tokenize("zzz port"), vec![1]);
    }

    #[test]
    fn similar_text_scores_higher() {
        let temp = tempfile::tempdir().unwrap();
        write_tiny_model(temp.path());
        let model = StaticModel::load(temp.path()).unwrap();

        let query = model.embed("port in use").unwrap();
        let close = model.embed("port use").unwrap();
        let far = model.embed("disk").unwrap();
        assert!(cosine(&query, &close) > cosine(&query, &far));
        assert!(model.embed("zzz").is_none());
    }
}

use std::{collections::HashSet, fs, path::Path};

use sentencepiece::SentencePieceProcessor;
use serde::Serialize;
use thiserror::Error;

const MAX_WAKE_PHRASE_CHARS: usize = 64;

#[derive(Debug, Error)]
pub enum WakeKeywordError {
    #[error("wake keyword resource is missing: {0}")]
    MissingResource(String),
    #[error("wake phrase is invalid: {0}")]
    InvalidPhrase(String),
    #[error("cannot load SentencePiece model: {0}")]
    Tokenizer(String),
    #[error("cannot read tokens vocabulary: {0}")]
    Tokens(String),
    #[error("wake phrase produced an unknown or unsupported token: {0}")]
    UnsupportedToken(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct PreparedWakeKeyword {
    pub phrase: String,
    pub canonical_label: String,
    pub tokens: Vec<String>,
    pub keyword_line: String,
}

/// Convert one English GigaSpeech wake phrase into the exact whitespace-separated
/// BPE representation expected by sherpa-onnx keyword spotting.
///
/// This function deliberately validates every SentencePiece result against the
/// runtime `tokens.txt`; a tokenizer result that the acoustic model cannot map
/// is rejected rather than written to `keywords.txt`.
pub fn prepare_gigaspeech_keyword(
    bpe_model: impl AsRef<Path>,
    tokens_path: impl AsRef<Path>,
    phrase: &str,
) -> Result<PreparedWakeKeyword, WakeKeywordError> {
    let bpe_model = bpe_model.as_ref();
    let tokens_path = tokens_path.as_ref();
    require_file("bpe.model", bpe_model)?;
    require_file("tokens.txt", tokens_path)?;

    let phrase = normalize_phrase(phrase)?;
    let processor = SentencePieceProcessor::open(bpe_model)
        .map_err(|error| WakeKeywordError::Tokenizer(error.to_string()))?;
    let vocabulary = read_vocabulary(tokens_path)?;
    let unknown_id = processor.unk_id();
    let encoded = processor
        .encode(&phrase)
        .map_err(|error| WakeKeywordError::Tokenizer(error.to_string()))?;

    if encoded.is_empty() {
        return Err(WakeKeywordError::InvalidPhrase(
            "SentencePiece produced no tokens".into(),
        ));
    }

    let mut tokens = Vec::with_capacity(encoded.len());
    for piece in encoded {
        if piece.id == unknown_id || piece.piece == "<unk>" {
            return Err(WakeKeywordError::UnsupportedToken(piece.piece));
        }
        if !vocabulary.contains(piece.piece.as_str()) {
            return Err(WakeKeywordError::UnsupportedToken(piece.piece));
        }
        tokens.push(piece.piece);
    }

    let canonical_label = phrase.replace(' ', "_");
    let keyword_line = format!("{} @{}", tokens.join(" "), canonical_label);
    Ok(PreparedWakeKeyword {
        phrase,
        canonical_label,
        tokens,
        keyword_line,
    })
}

fn normalize_phrase(value: &str) -> Result<String, WakeKeywordError> {
    let phrase = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase();

    if phrase.is_empty() {
        return Err(WakeKeywordError::InvalidPhrase(
            "phrase must not be empty".into(),
        ));
    }
    if phrase.chars().count() > MAX_WAKE_PHRASE_CHARS {
        return Err(WakeKeywordError::InvalidPhrase(format!(
            "phrase must be at most {MAX_WAKE_PHRASE_CHARS} characters"
        )));
    }
    if !phrase
        .chars()
        .all(|character| character.is_ascii_alphabetic() || character == ' ' || character == '\'')
    {
        return Err(WakeKeywordError::InvalidPhrase(
            "the current GigaSpeech wake model accepts English letters, spaces and apostrophes only"
                .into(),
        ));
    }

    Ok(phrase)
}

fn read_vocabulary(path: &Path) -> Result<HashSet<String>, WakeKeywordError> {
    let text = fs::read_to_string(path)
        .map_err(|error| WakeKeywordError::Tokens(error.to_string()))?;
    let mut vocabulary = HashSet::new();

    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(token) = fields.next() else {
            continue;
        };
        let Some(id) = fields.next() else {
            return Err(WakeKeywordError::Tokens(format!(
                "invalid tokens.txt line {}: expected `<token> <id>`",
                line_number + 1
            )));
        };
        if fields.next().is_some() || id.parse::<u32>().is_err() {
            return Err(WakeKeywordError::Tokens(format!(
                "invalid tokens.txt line {}",
                line_number + 1
            )));
        }
        vocabulary.insert(token.to_owned());
    }

    if vocabulary.is_empty() {
        return Err(WakeKeywordError::Tokens(
            "tokens.txt contains no vocabulary entries".into(),
        ));
    }
    Ok(vocabulary)
}

fn require_file(name: &str, path: &Path) -> Result<(), WakeKeywordError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(WakeKeywordError::MissingResource(format!(
            "{name}: {}",
            path.display()
        )))
    }
}

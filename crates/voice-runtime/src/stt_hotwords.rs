use std::{collections::HashSet, fs, path::Path};

use sentencepiece::SentencePieceProcessor;
use thiserror::Error;

pub const STT_HOTWORDS_FILE: &str = "hotwords.txt";
const MAX_HOTWORDS_FILE_BYTES: u64 = 16 * 1024;
const MAX_HOTWORD_PHRASES: usize = 128;
const MAX_HOTWORD_CHARS: usize = 128;

#[derive(Debug, Error)]
pub enum SttHotwordError {
    #[error("cannot inspect STT hotwords file: {0}")]
    Metadata(String),
    #[error("STT hotwords file is too large: maximum is {MAX_HOTWORDS_FILE_BYTES} bytes")]
    FileTooLarge,
    #[error("cannot read STT hotwords file: {0}")]
    Read(String),
    #[error("STT hotword resource is missing: {0}")]
    MissingResource(String),
    #[error("cannot load SentencePiece model: {0}")]
    Tokenizer(String),
    #[error("cannot read Zipformer token vocabulary: {0}")]
    Tokens(String),
    #[error("STT hotwords file contains more than {MAX_HOTWORD_PHRASES} phrases")]
    TooManyPhrases,
}

#[derive(Debug, Clone)]
pub struct RejectedHotword {
    pub phrase: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct PreparedHotwords {
    /// User-facing phrases that were successfully encoded.
    pub phrases: Vec<String>,
    /// Sherpa per-stream format: BPE pieces separated by spaces and phrases by `/`.
    pub encoded: String,
    /// Invalid or unsupported phrases are skipped instead of disabling STT entirely.
    pub rejected: Vec<RejectedHotword>,
}

impl PreparedHotwords {
    pub fn is_empty(&self) -> bool {
        self.encoded.is_empty()
    }
}

/// Load a user-managed hotword file and convert plain phrases into the tokenized
/// representation accepted by sherpa-onnx `create_stream_with_hotwords`.
///
/// Missing `hotwords.txt` is not an error: it means contextual biasing is off.
/// Resource/tokenizer failures are reported so callers can log them and safely
/// fall back to ordinary greedy decoding.
pub fn load_prepared_hotwords(
    hotwords_path: impl AsRef<Path>,
    bpe_model: impl AsRef<Path>,
    tokens_path: impl AsRef<Path>,
) -> Result<Option<PreparedHotwords>, SttHotwordError> {
    let hotwords_path = hotwords_path.as_ref();
    if !hotwords_path.exists() {
        return Ok(None);
    }

    let metadata = fs::metadata(hotwords_path)
        .map_err(|error| SttHotwordError::Metadata(error.to_string()))?;
    if !metadata.is_file() {
        return Err(SttHotwordError::Read(format!(
            "{} is not a regular file",
            hotwords_path.display()
        )));
    }
    if metadata.len() > MAX_HOTWORDS_FILE_BYTES {
        return Err(SttHotwordError::FileTooLarge);
    }

    let text = fs::read_to_string(hotwords_path)
        .map_err(|error| SttHotwordError::Read(error.to_string()))?;
    let phrases = parse_phrases(&text)?;
    if phrases.is_empty() {
        return Ok(None);
    }

    let bpe_model = bpe_model.as_ref();
    let tokens_path = tokens_path.as_ref();
    require_file("bpe.model", bpe_model)?;
    require_file("tokens.txt", tokens_path)?;

    let processor = SentencePieceProcessor::open(bpe_model)
        .map_err(|error| SttHotwordError::Tokenizer(error.to_string()))?;
    let vocabulary = read_vocabulary(tokens_path)?;
    let unknown_id = processor.unk_id();

    let mut accepted_phrases = Vec::with_capacity(phrases.len());
    let mut encoded_phrases = Vec::with_capacity(phrases.len());
    let mut rejected = Vec::new();
    let mut seen_encoded = HashSet::<String>::new();

    for phrase in phrases {
        match encode_phrase(&processor, &vocabulary, unknown_id, &phrase) {
            Ok(encoded) => {
                if seen_encoded.insert(encoded.clone()) {
                    accepted_phrases.push(phrase);
                    encoded_phrases.push(encoded);
                }
            }
            Err(reason) => rejected.push(RejectedHotword { phrase, reason }),
        }
    }

    Ok(Some(PreparedHotwords {
        phrases: accepted_phrases,
        encoded: encoded_phrases.join("/"),
        rejected,
    }))
}

fn parse_phrases(text: &str) -> Result<Vec<String>, SttHotwordError> {
    let mut phrases = Vec::new();
    let mut seen = HashSet::<String>::new();

    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let phrase = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
        let count = phrase.chars().count();
        if count == 0 {
            continue;
        }
        if count > MAX_HOTWORD_CHARS {
            // A single malformed line should not make every other hotword useless.
            // Keep it in the set so token preparation can report it as rejected.
            let marker = format!("__TOO_LONG__{phrase}");
            if seen.insert(marker) {
                phrases.push(phrase);
            }
        } else if phrase.contains('/') || phrase.contains('\0') {
            let marker = format!("__INVALID_SEPARATOR__{phrase}");
            if seen.insert(marker) {
                phrases.push(phrase);
            }
        } else if seen.insert(phrase.clone()) {
            phrases.push(phrase);
        }

        if phrases.len() > MAX_HOTWORD_PHRASES {
            return Err(SttHotwordError::TooManyPhrases);
        }
    }

    Ok(phrases)
}

fn encode_phrase(
    processor: &SentencePieceProcessor,
    vocabulary: &HashSet<String>,
    unknown_id: u32,
    phrase: &str,
) -> Result<String, String> {
    if phrase.chars().count() > MAX_HOTWORD_CHARS {
        return Err(format!(
            "phrase exceeds the {MAX_HOTWORD_CHARS}-character limit"
        ));
    }
    if phrase.contains('/') {
        return Err("`/` is reserved as the Sherpa hotword phrase separator".into());
    }
    if phrase.contains('\0') {
        return Err("NUL is not allowed in a hotword phrase".into());
    }

    let encoded = processor
        .encode(phrase)
        .map_err(|error| format!("SentencePiece encode failed: {error}"))?;
    if encoded.is_empty() {
        return Err("SentencePiece produced no tokens".into());
    }

    let mut pieces = Vec::with_capacity(encoded.len());
    for piece in encoded {
        if piece.id == unknown_id || piece.piece == "<unk>" {
            return Err(format!("unsupported token `{}`", piece.piece));
        }
        if !vocabulary.contains(piece.piece.as_str()) {
            return Err(format!(
                "token `{}` is not present in Zipformer tokens.txt",
                piece.piece
            ));
        }
        pieces.push(piece.piece);
    }

    Ok(pieces.join(" "))
}

fn read_vocabulary(path: &Path) -> Result<HashSet<String>, SttHotwordError> {
    let text = fs::read_to_string(path)
        .map_err(|error| SttHotwordError::Tokens(error.to_string()))?;
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
            return Err(SttHotwordError::Tokens(format!(
                "invalid tokens.txt line {}: expected `<token> <id>`",
                line_number + 1
            )));
        };
        if fields.next().is_some() || id.parse::<u32>().is_err() {
            return Err(SttHotwordError::Tokens(format!(
                "invalid tokens.txt line {}",
                line_number + 1
            )));
        }
        vocabulary.insert(token.to_owned());
    }

    if vocabulary.is_empty() {
        return Err(SttHotwordError::Tokens(
            "tokens.txt contains no vocabulary entries".into(),
        ));
    }
    Ok(vocabulary)
}

fn require_file(name: &str, path: &Path) -> Result<(), SttHotwordError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(SttHotwordError::MissingResource(format!(
            "{name}: {}",
            path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_ignores_comments_blank_lines_and_duplicates() {
        let phrases = parse_phrases(
            "\n# assistant vocabulary\nVisual Studio Code\n  SQL   Server  \nVisual Studio Code\n",
        )
        .expect("parse hotwords");

        assert_eq!(phrases, vec!["Visual Studio Code", "SQL Server"]);
    }

    #[test]
    fn parser_rejects_unbounded_phrase_counts() {
        let text = (0..=MAX_HOTWORD_PHRASES)
            .map(|index| format!("phrase {index}"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(matches!(
            parse_phrases(&text),
            Err(SttHotwordError::TooManyPhrases)
        ));
    }
}

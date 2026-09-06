use std::{
    collections::HashSet,
    fs,
    io,
    path::Path,
};

use sentencepiece::SentencePieceProcessor;
use thiserror::Error;

pub const STT_HOTWORDS_FILE: &str = "hotwords.txt";
pub const GENERATED_BPE_VOCAB_FILE: &str = "bpe.vocab.generated";
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
    #[error("cannot export Sherpa BPE vocabulary: {0}")]
    BpeVocab(String),
}

#[derive(Debug, Clone)]
pub struct RejectedHotword {
    pub phrase: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct PreparedHotwords {
    /// User-facing phrases that were successfully validated.
    pub phrases: Vec<String>,
    /// Sherpa per-stream format. Native SentencePiece tokenization is enabled
    /// through `modeling_unit=bpe` and the generated BPE vocabulary.
    pub inline: String,
    /// Invalid or unsupported phrases are skipped instead of disabling STT entirely.
    pub rejected: Vec<RejectedHotword>,
}

impl PreparedHotwords {
    pub fn is_empty(&self) -> bool {
        self.inline.is_empty()
    }
}

/// Load a user-managed hotword file and validate every phrase against the same
/// SentencePiece model and token vocabulary used by the Vietnamese Zipformer.
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

    let mut accepted = Vec::with_capacity(phrases.len());
    let mut rejected = Vec::new();

    for phrase in phrases {
        match validate_phrase(&processor, &vocabulary, unknown_id, &phrase) {
            Ok(()) => accepted.push(phrase),
            Err(reason) => rejected.push(RejectedHotword { phrase, reason }),
        }
    }

    Ok(Some(PreparedHotwords {
        inline: accepted.join("/"),
        phrases: accepted,
        rejected,
    }))
}

/// Sherpa's native BPE hotword encoder expects the two-column vocabulary that
/// SentencePiece normally emits next to `bpe.model`. The Vietnamese package only
/// ships `bpe.model`, so export the required vocabulary locally on first use.
///
/// SentencePiece `.model` is a protobuf `ModelProto`; field 1 is the repeated
/// `SentencePiece` message, whose fields 1 and 2 are the piece string and f32
/// score. We parse only those public protobuf fields and skip everything else.
pub fn ensure_generated_bpe_vocab(
    bpe_model: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<(), SttHotwordError> {
    let bpe_model = bpe_model.as_ref();
    let output_path = output_path.as_ref();
    require_file("bpe.model", bpe_model)?;

    if output_path
        .metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
    {
        return Ok(());
    }

    let model = fs::read(bpe_model).map_err(|error| SttHotwordError::BpeVocab(error.to_string()))?;
    let entries = parse_sentencepiece_vocab(&model)?;
    if entries.is_empty() {
        return Err(SttHotwordError::BpeVocab(
            "bpe.model contains no SentencePiece entries".into(),
        ));
    }

    let mut text = String::with_capacity(entries.len() * 16);
    for (piece, score) in entries {
        if piece.is_empty() || piece.chars().any(char::is_whitespace) {
            return Err(SttHotwordError::BpeVocab(format!(
                "SentencePiece vocabulary contains an invalid whitespace token: {piece:?}"
            )));
        }
        if !score.is_finite() {
            return Err(SttHotwordError::BpeVocab(format!(
                "SentencePiece vocabulary contains a non-finite score for {piece:?}"
            )));
        }
        text.push_str(&piece);
        text.push('\t');
        text.push_str(&score.to_string());
        text.push('\n');
    }

    let parent = output_path.parent().ok_or_else(|| {
        SttHotwordError::BpeVocab(format!("invalid output path: {}", output_path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|error| SttHotwordError::BpeVocab(error.to_string()))?;

    let temp_path = output_path.with_extension("generated.part");
    fs::write(&temp_path, text).map_err(|error| SttHotwordError::BpeVocab(error.to_string()))?;

    if output_path.exists() {
        fs::remove_file(output_path)
            .map_err(|error| SttHotwordError::BpeVocab(error.to_string()))?;
    }
    fs::rename(&temp_path, output_path)
        .map_err(|error| SttHotwordError::BpeVocab(error.to_string()))?;
    Ok(())
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
        if phrase.is_empty() || !seen.insert(phrase.clone()) {
            continue;
        }
        phrases.push(phrase);
        if phrases.len() > MAX_HOTWORD_PHRASES {
            return Err(SttHotwordError::TooManyPhrases);
        }
    }

    Ok(phrases)
}

fn validate_phrase(
    processor: &SentencePieceProcessor,
    vocabulary: &HashSet<String>,
    unknown_id: u32,
    phrase: &str,
) -> Result<(), String> {
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
    if phrase
        .split_whitespace()
        .any(|word| word.starts_with(':') || word.starts_with('#') || word.starts_with('@'))
    {
        return Err("tokens beginning with `:`, `#`, or `@` are reserved by Sherpa".into());
    }

    // Sherpa's BPE hotword encoder tokenizes each whitespace-separated word
    // independently, so mirror that boundary during validation.
    for word in phrase.split_whitespace() {
        let encoded = processor
            .encode(word)
            .map_err(|error| format!("SentencePiece encode failed: {error}"))?;
        if encoded.is_empty() {
            return Err(format!("SentencePiece produced no tokens for `{word}`"));
        }

        for piece in encoded {
            if piece.id == unknown_id || piece.piece == "<unk>" {
                return Err(format!("unsupported token in `{word}`: `{}`", piece.piece));
            }
            if !vocabulary.contains(piece.piece.as_str()) {
                return Err(format!(
                    "token `{}` from `{word}` is not present in Zipformer tokens.txt",
                    piece.piece
                ));
            }
        }
    }

    Ok(())
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

fn parse_sentencepiece_vocab(model: &[u8]) -> Result<Vec<(String, f32)>, SttHotwordError> {
    let mut cursor = 0usize;
    let mut entries = Vec::new();

    while cursor < model.len() {
        let key = read_varint(model, &mut cursor)?;
        let field = key >> 3;
        let wire = (key & 0x07) as u8;

        if field == 1 && wire == 2 {
            let message = read_length_delimited(model, &mut cursor)?;
            entries.push(parse_sentencepiece_entry(message)?);
        } else {
            skip_field(model, &mut cursor, wire)?;
        }
    }

    Ok(entries)
}

fn parse_sentencepiece_entry(message: &[u8]) -> Result<(String, f32), SttHotwordError> {
    let mut cursor = 0usize;
    let mut piece: Option<String> = None;
    let mut score = 0.0f32;

    while cursor < message.len() {
        let key = read_varint(message, &mut cursor)?;
        let field = key >> 3;
        let wire = (key & 0x07) as u8;

        match (field, wire) {
            (1, 2) => {
                let value = read_length_delimited(message, &mut cursor)?;
                piece = Some(
                    std::str::from_utf8(value)
                        .map_err(|error| SttHotwordError::BpeVocab(error.to_string()))?
                        .to_owned(),
                );
            }
            (2, 5) => {
                let bytes = take_bytes(message, &mut cursor, 4)?;
                score = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            }
            _ => skip_field(message, &mut cursor, wire)?,
        }
    }

    let piece = piece.ok_or_else(|| {
        SttHotwordError::BpeVocab("SentencePiece entry is missing its piece string".into())
    })?;
    Ok((piece, score))
}

fn read_varint(bytes: &[u8], cursor: &mut usize) -> Result<u64, SttHotwordError> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes.get(*cursor).ok_or_else(|| {
            SttHotwordError::BpeVocab("truncated protobuf varint".into())
        })?;
        *cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(SttHotwordError::BpeVocab(
        "protobuf varint exceeds 64 bits".into(),
    ))
}

fn read_length_delimited<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
) -> Result<&'a [u8], SttHotwordError> {
    let length = read_varint(bytes, cursor)?;
    let length = usize::try_from(length)
        .map_err(|_| SttHotwordError::BpeVocab("protobuf length is too large".into()))?;
    take_bytes(bytes, cursor, length)
}

fn take_bytes<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], SttHotwordError> {
    let end = cursor
        .checked_add(length)
        .ok_or_else(|| SttHotwordError::BpeVocab("protobuf length overflow".into()))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| SttHotwordError::BpeVocab("truncated protobuf field".into()))?;
    *cursor = end;
    Ok(value)
}

fn skip_field(bytes: &[u8], cursor: &mut usize, wire: u8) -> Result<(), SttHotwordError> {
    match wire {
        0 => {
            let _ = read_varint(bytes, cursor)?;
            Ok(())
        }
        1 => {
            let _ = take_bytes(bytes, cursor, 8)?;
            Ok(())
        }
        2 => {
            let _ = read_length_delimited(bytes, cursor)?;
            Ok(())
        }
        5 => {
            let _ = take_bytes(bytes, cursor, 4)?;
            Ok(())
        }
        3 | 4 => Err(SttHotwordError::BpeVocab(
            "protobuf groups are not supported in SentencePiece model parsing".into(),
        )),
        _ => Err(SttHotwordError::BpeVocab(format!(
            "unsupported protobuf wire type {wire}"
        ))),
    }
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

    #[test]
    fn extracts_piece_and_score_from_minimal_model_proto() {
        // ModelProto { pieces: [SentencePiece { piece: "▁xin", score: -1.25 }] }
        let piece = "▁xin".as_bytes();
        let mut entry = vec![0x0a, piece.len() as u8];
        entry.extend_from_slice(piece);
        entry.push(0x15);
        entry.extend_from_slice(&(-1.25f32).to_le_bytes());

        let mut model = vec![0x0a, entry.len() as u8];
        model.extend_from_slice(&entry);

        let parsed = parse_sentencepiece_vocab(&model).expect("parse model proto");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "▁xin");
        assert!((parsed[0].1 + 1.25).abs() < f32::EPSILON);
    }
}

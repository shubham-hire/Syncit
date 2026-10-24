//! Shared helpers for the AudioSync command-line prototypes: a tiny flag
//! parser, hex session/token encoding, and the CPAL output sink.

pub mod cpal_sink;

use std::collections::{HashMap, HashSet};
use std::fmt;

/// Minimal `--key value` / `--flag` parser matching the smoke-test style.
#[derive(Debug, Default)]
pub struct Args {
    values: HashMap<String, String>,
    flags: HashSet<String>,
}

impl Args {
    /// Parses arguments after the program name. Every `--key` must appear in
    /// `known_values` (expects a value) or `known_flags` (stands alone).
    pub fn parse<I, S>(
        arguments: I,
        known_values: &[&str],
        known_flags: &[&str],
    ) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut values = HashMap::new();
        let mut flags = HashSet::new();
        let mut iterator = arguments.into_iter().map(Into::into).peekable();

        while let Some(argument) = iterator.next() {
            let Some(key) = argument.strip_prefix("--") else {
                return Err(format!(
                    "unrecognized argument {argument:?}; expected --flags only"
                ));
            };
            if key == "help" || key == "h" {
                return Err(String::new());
            }
            if known_flags.contains(&key) {
                flags.insert(key.to_owned());
                continue;
            }
            if known_values.contains(&key) {
                let value = iterator
                    .next()
                    .ok_or_else(|| format!("--{key} requires a value"))?;
                values.insert(key.to_owned(), value);
                continue;
            }
            return Err(format!(
                "unrecognized argument --{key}; known values: {:?}, known flags: {:?}",
                known_values, known_flags
            ));
        }

        Ok(Self { values, flags })
    }

    pub fn value(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }

    pub fn required_value(&self, key: &str) -> Result<&str, String> {
        self.value(key)
            .ok_or_else(|| format!("--{key} is required"))
    }
}

/// Renders 16 bytes as lowercase hex.
pub fn hex16(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Parses 32 hex characters into 16 bytes.
pub fn parse_hex16(text: &str) -> Result<[u8; 16], String> {
    if text.len() != 32 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "expected 32 hex characters for a 16-byte value, got {text:?}"
        ));
    }
    let mut bytes = [0_u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|error| format!("invalid hex value: {error}"))?;
    }
    Ok(bytes)
}

/// Generates 16 bytes of prototype-grade randomness using the standard
/// library's OS-seeded hasher states. Not a cryptographic source; sufficient
/// for a local session token until authenticated traffic lands.
pub fn random_bytes16() -> [u8; 16] {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let mut bytes = [0_u8; 16];
    for chunk in bytes.chunks_mut(8) {
        let hashed = RandomState::new().build_hasher().finish().to_be_bytes();
        let length = chunk.len();
        chunk.copy_from_slice(&hashed[..length]);
    }
    bytes
}

/// A terminated command-line run.
#[derive(Debug)]
pub struct CliError {
    pub message: String,
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for CliError {}

pub type CliResult<T> = Result<T, CliError>;

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_parse_values_flags_and_reject_unknowns() {
        let args = Args::parse(
            [
                "--host",
                "127.0.0.1:41000",
                "--synthetic",
                "--delay-ms",
                "40",
            ],
            &["host", "delay-ms"],
            &["synthetic"],
        )
        .expect("known arguments should parse");
        assert_eq!(args.value("host"), Some("127.0.0.1:41000"));
        assert_eq!(args.value("delay-ms"), Some("40"));
        assert!(args.flag("synthetic"));
        assert!(!args.flag("host"));

        let error = Args::parse(["--nope"], &["host"], &["synthetic"])
            .expect_err("unknown arguments must be rejected");
        assert!(error.contains("--nope"));
        assert!(Args::parse(["--host"], &["host"], &[]).is_err());
    }

    #[test]
    fn hex16_round_trips() {
        let bytes = random_bytes16();
        let text = hex16(&bytes);
        assert_eq!(text.len(), 32);
        assert_eq!(parse_hex16(&text), Ok(bytes));
        assert!(parse_hex16("xyz").is_err());
        assert!(parse_hex16(&"0".repeat(31)).is_err());
    }
}

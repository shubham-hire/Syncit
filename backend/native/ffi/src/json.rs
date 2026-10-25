//! Minimal JSON writer/reader for the FFI boundary.
//!
//! Only what the bridge needs: flat objects of string/number/bool fields and
//! string arrays, plus tolerant `key` lookup for scalar values. Input comes
//! from our own Dart layer, so the reader trades strictness for compactness.

/// Builds a flat JSON object and string arrays.
pub struct JsonWriter {
    buffer: String,
    first: bool,
}

impl Default for JsonWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonWriter {
    pub fn new() -> Self {
        Self {
            buffer: String::from("{"),
            first: true,
        }
    }

    fn comma(&mut self) {
        if !self.first {
            self.buffer.push(',');
        }
        self.first = false;
    }

    /// Adds a raw (pre-rendered) field for numbers and booleans.
    pub fn raw(&mut self, key: &str, value: &str) {
        self.comma();
        self.buffer.push('"');
        escape_into(key, &mut self.buffer);
        self.buffer.push_str("\":");
        self.buffer.push_str(value);
    }

    /// Adds a string field. `value` is escaped.
    pub fn field(&mut self, key: &str, value: &str) {
        self.comma();
        self.buffer.push('"');
        escape_into(key, &mut self.buffer);
        self.buffer.push_str("\":\"");
        escape_into(value, &mut self.buffer);
        self.buffer.push('"');
    }

    /// Adds an array-of-strings field. Elements are raw JSON.
    pub fn array(&mut self, key: &str, elements: &[String]) {
        self.comma();
        self.buffer.push('"');
        escape_into(key, &mut self.buffer);
        self.buffer.push_str("\":[");
        for (index, element) in elements.iter().enumerate() {
            if index > 0 {
                self.buffer.push(',');
            }
            self.buffer.push('"');
            escape_into(element, &mut self.buffer);
            self.buffer.push('"');
        }
        self.buffer.push(']');
    }

    /// Closes the object and returns the JSON text.
    pub fn finish(mut self) -> String {
        self.buffer.push('}');
        self.buffer
    }
}

fn escape_into(text: &str, buffer: &mut String) {
    for character in text.chars() {
        match character {
            '"' => buffer.push_str("\\\""),
            '\\' => buffer.push_str("\\\\"),
            '\n' => buffer.push_str("\\n"),
            '\r' => buffer.push_str("\\r"),
            '\t' => buffer.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                buffer.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => buffer.push(c),
        }
    }
}

/// Reads a string field's raw value (unescaped) from flat JSON.
pub fn get_str<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\":");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let rest = rest.trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let end = rest[1..].find('"')? + 1;
    Some(&rest[1..end])
}

/// Reads a numeric field as u64.
pub fn get_u64(text: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\":");
    let start = text.find(&needle)? + needle.len();
    let rest = text[start..].trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// Reads a boolean field.
pub fn get_bool(text: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{key}\":");
    let start = text.find(&needle)? + needle.len();
    let rest = text[start..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

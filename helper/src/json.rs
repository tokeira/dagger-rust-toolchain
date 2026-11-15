//! A reader for the JSON that Cargo and cargo-nextest print: enough to find
//! the paths they name. The helper depends on nothing but the standard
//! library, so that building it needs no crate and no network.

/// A JSON value. A number keeps its text: the helper reads none.
#[derive(Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// The text of a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    /// The values of an object's members, or the items of an array.
    pub fn values(&self) -> Vec<&Value> {
        match self {
            Value::Object(members) => members.iter().map(|(_, value)| value).collect(),
            Value::Array(items) => items.iter().collect(),
            _ => Vec::new(),
        }
    }
}

/// How deep a document may nest. Cargo's and cargo-nextest's stay far below.
const DEPTH: usize = 128;

/// Reads one JSON document.
pub fn parse(text: &str) -> Result<Value, String> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = reader.value(0)?;
    reader.space();
    if reader.at == reader.bytes.len() {
        Ok(value)
    } else {
        Err(reader.wrong("text after the document"))
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn wrong(&self, what: &str) -> String {
        format!("JSON: {what} at byte {}", self.at)
    }

    fn space(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.wrong("an unknown word"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > DEPTH {
            return Err(self.wrong("too deep a document"));
        }
        self.space();
        match self.bytes.get(self.at) {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => Ok(self.number()),
            Some(_) => Err(self.wrong("an unexpected character")),
            None => Err(self.wrong("the end of the document")),
        }
    }

    fn number(&mut self) -> Value {
        let start = self.at;
        while matches!(
            self.bytes.get(self.at),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.at += 1;
        }
        Value::Number(String::from_utf8_lossy(&self.bytes[start..self.at]).into_owned())
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.at += 1;
        let mut items = Vec::new();
        self.space();
        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.space();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.wrong("an array without its end")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.at += 1;
        let mut members = Vec::new();
        self.space();
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.space();
            if self.bytes.get(self.at) != Some(&b'"') {
                return Err(self.wrong("a member without a name"));
            }
            let name = self.string()?;
            self.space();
            if self.bytes.get(self.at) != Some(&b':') {
                return Err(self.wrong("a member without a value"));
            }
            self.at += 1;
            members.push((name, self.value(depth + 1)?));
            self.space();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(self.wrong("an object without its end")),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut text = Vec::new();
        loop {
            let Some(&byte) = self.bytes.get(self.at) else {
                return Err(self.wrong("a string without its end"));
            };
            self.at += 1;
            match byte {
                b'"' => {
                    return String::from_utf8(text)
                        .map_err(|_| self.wrong("a string that is not UTF-8"));
                }
                b'\\' => {
                    let Some(&escaped) = self.bytes.get(self.at) else {
                        return Err(self.wrong("a string without its end"));
                    };
                    self.at += 1;
                    let character = match escaped {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.escaped()?,
                        _ => return Err(self.wrong("an unknown escape")),
                    };
                    text.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
                }
                _ => text.push(byte),
            }
        }
    }

    /// The character of a `\u` escape, which may take a second escape for
    /// the low half of a surrogate pair.
    fn escaped(&mut self) -> Result<char, String> {
        let high = self.hex()?;
        let code = if (0xd800..0xdc00).contains(&high) {
            if !self.bytes[self.at..].starts_with(b"\\u") {
                return Err(self.wrong("half a surrogate pair"));
            }
            self.at += 2;
            let low = self.hex()?;
            if !(0xdc00..0xe000).contains(&low) {
                return Err(self.wrong("half a surrogate pair"));
            }
            0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
        } else {
            high
        };
        char::from_u32(code).ok_or_else(|| self.wrong("an escape that is no character"))
    }

    fn hex(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .ok_or_else(|| self.wrong("a short escape"))?;
        let code = u32::from_str_radix(digits, 16).map_err(|_| self.wrong("a wrong escape"))?;
        self.at += 4;
        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Value};

    #[test]
    fn reads_every_kind_of_value() {
        let document =
            parse(r#" {"a": [1, -2.5e3, true, false, null], "b": {"c": "d"}, "e": [], "f": {}} "#)
                .unwrap();
        let items = document.get("a").unwrap().values();
        assert_eq!(items[0], &Value::Number("1".to_owned()));
        assert_eq!(items[1], &Value::Number("-2.5e3".to_owned()));
        assert_eq!(items[2], &Value::Bool(true));
        assert_eq!(items[4], &Value::Null);
        assert_eq!(
            document.get("b").unwrap().get("c").unwrap().as_str(),
            Some("d")
        );
        assert!(document.get("e").unwrap().values().is_empty());
        assert!(document.get("f").unwrap().values().is_empty());
        assert!(document.get("missing").is_none());
    }

    #[test]
    fn reads_escapes() {
        let document = parse(r#""a\"b\\c\/d\n\té😀 é""#).unwrap();
        assert_eq!(document.as_str(), Some("a\"b\\c/d\n\t\u{e9}\u{1f600} é"));
    }

    #[test]
    fn refuses_what_is_not_a_document() {
        for text in [
            "",
            "{",
            "[1,",
            r#"{"a" 1}"#,
            r#""open"#,
            "nul",
            "{} {}",
            r#""\ud83d""#,
            r#""\x""#,
        ] {
            assert!(parse(text).is_err(), "{text:?} must not parse");
        }
    }

    #[test]
    fn refuses_a_document_that_nests_too_deep() {
        assert!(parse(&"[".repeat(100_000)).is_err());
    }
}

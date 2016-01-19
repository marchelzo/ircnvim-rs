use std::io::Write;
use std::io;
use std::str;

#[derive(Debug, Clone)]
pub struct TextChunk {
    pub text: String,
    pub fg: u8,
    pub bg: u8,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

#[derive(Debug, Clone)]
pub struct Text {
    raw: String,
    chunks: Vec<TextChunk>,
    pub ctcp: bool,
    pub action: bool
}

const DEFAULT_COLOR: u8 = 255;

fn parse_color<I>(bytes: &mut I) -> (Option<u8>, Option<u8>) where I: Iterator<Item=u8> {
    (None, None)
}

impl Text {
    pub fn from_bytes(mut bytes: Vec<u8>) -> Text {

        let mut raw = String::new();
        let mut chunks = Vec::new();
        let mut ctcp = false;
        let mut action = false;

        let mut bytes = if bytes[0] == 0x01 {
            ctcp = true;
            bytes.remove(0);
            let n = bytes.len();
            bytes.remove(n - 1);
            if &bytes[..6] == b"ACTION" {
                action = true;
                bytes.into_iter().skip(7)
            } else {
                bytes.into_iter().skip(0) // TODO: handle CTCP messages other than ACTIONs
            }
        } else {
            bytes.into_iter().skip(0)
        };

        let mut bold = false;
        let mut italic = false;
        let mut underline = false;
        let mut reverse = false;

        let mut fg = DEFAULT_COLOR;
        let mut bg = DEFAULT_COLOR;

        let mut chunk = Vec::new();
        loop {
            let b = bytes.next();
            let mut update = true;
            match b {
                Some(0x02) => { bold = !bold },
                Some(0x1D) => { italic = !italic },
                Some(0x1F) => { underline = !underline },
                Some(0x16) => { reverse = !reverse },
                Some(0x0F) => { 
                    bold = false;
                    italic = false;
                    underline = false;
                    reverse = false;
                    fg = DEFAULT_COLOR;
                    bg = DEFAULT_COLOR;
                },
                Some(0x03) => {
                    match parse_color(&mut bytes) {
                        (None, None)       => { fg = DEFAULT_COLOR; bg = DEFAULT_COLOR }
                        (Some(f), None)    => { fg = f },
                        (None, Some(b))    => { bg = b },
                        (Some(f), Some(b)) => { fg = f; bg = b }
                    }
                },
                Some(b)    => { chunk.push(b); update = false; }
                _          => { }
            }

            if update {
                let s: String = String::from_utf8(chunk).expect("from utf8");
                raw.push_str(&s);
                chunks.push(TextChunk {
                    text: s,
                    fg: fg,
                    bg: bg,
                    bold: bold,
                    italic: italic,
                    underline: underline,
                    reverse: reverse
                });

                chunk = Vec::new();
            }

            if b.is_none() { break }
        }

        return Text {
            raw: raw,
            chunks: chunks,
            ctcp: ctcp,
            action: action
        };
    }

    pub fn from_string(s: String) -> Text {
        return Text {
            raw: s.clone(),
            ctcp: false,
            action: false,
            chunks: vec![TextChunk {
                text: s,
                fg: DEFAULT_COLOR,
                bg: DEFAULT_COLOR,
                bold: false,
                italic: false,
                underline: false,
                reverse: false
            }]
        };
    }
    pub fn action(s: String) -> Text {
        return Text {
            raw: s.clone(),
            ctcp: true,
            action: true,
            chunks: vec![TextChunk {
                text: s,
                fg: DEFAULT_COLOR,
                bg: DEFAULT_COLOR,
                bold: false,
                italic: false,
                underline: false,
                reverse: false
            }]
        };
    }

    pub fn text(&self) -> &str {
        return &self.raw;
    }

    pub fn chunks(&self) -> &[TextChunk] {
        return &self.chunks[..];
    }

    pub fn decorate_nick(nick: &str) -> Text {
        return Text::from_string(format!("<{}>", nick));
    }
}

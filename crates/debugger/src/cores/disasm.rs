use std::borrow::Cow;

use cirsium::widgets::{DisasmToken, TokenKind};
use logos::Logos;

pub struct SyntaxSpec {
    pub registers: &'static [&'static str],
    pub symbols: fn(&str) -> bool,
}

pub fn strip_symbol_sigils<'a>(text: &'a str, syntax: &SyntaxSpec) -> Cow<'a, str> {
    let bytes = text.as_bytes();
    let mut out: Option<String> = None;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }

            if end > start && (syntax.symbols)(&text[start..end]) {
                out.get_or_insert_with(|| {
                    let mut s = String::with_capacity(text.len());
                    s.push_str(&text[..i]);
                    s
                })
                .push_str(&text[start..end]);
                i = end;
                continue;
            }
        }

        if let Some(out) = out.as_mut() {
            out.push(bytes[i] as char);
        }
        i += 1;
    }

    out.map_or(Cow::Borrowed(text), Cow::Owned)
}

#[derive(Logos)]
#[logos(skip r"[ \t]+")]
enum Operand {
    #[token("#")]
    Imm,
    #[regex(r"\$[0-9a-fA-F]*")]
    Hex,
    #[regex(r"[0-9]+")]
    Dec,
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,
    #[regex(r"[,()\[\]]")]
    Punct,
}

pub fn tokenize(text: &str, mnemonic: TokenKind, syntax: &SyntaxSpec) -> Vec<DisasmToken> {
    let mut tokens = Vec::with_capacity(8);

    let split = text
        .bytes()
        .position(|b| !b.is_ascii_alphabetic())
        .unwrap_or(text.len());
    if split > 0 {
        tokens.push(DisasmToken::new(&text[..split], mnemonic));
    }

    let mut lexer = Operand::lexer(&text[split..]);
    while let Some(result) = lexer.next() {
        let slice = lexer.slice();
        match result {
            Ok(Operand::Imm) => tokens.push(DisasmToken::new("#", TokenKind::ImmPrefix)),
            Ok(Operand::Hex) => {
                tokens.push(DisasmToken::new("$", TokenKind::HexPrefix));
                if slice.len() > 1 {
                    tokens.push(DisasmToken::new(&slice[1..], TokenKind::Number));
                }
            }
            Ok(Operand::Dec) => tokens.push(DisasmToken::new(slice, TokenKind::Number)),
            Ok(Operand::Ident) => {
                let kind = if syntax
                    .registers
                    .iter()
                    .any(|r| r.eq_ignore_ascii_case(slice))
                {
                    TokenKind::Register
                } else if (syntax.symbols)(slice) {
                    TokenKind::Symbol
                } else {
                    TokenKind::Plain
                };
                tokens.push(DisasmToken::new(slice, kind));
            }
            Ok(Operand::Punct) => tokens.push(DisasmToken::new(slice, TokenKind::Punctuation)),
            Err(()) => tokens.push(DisasmToken::new(slice, TokenKind::Plain)),
        }
    }

    tokens
}

use std::borrow::Cow;
use std::ops::Range;

use logos::Logos;

use crate::model::{DisasmToken, TokenKind};

pub struct SyntaxSpec {
    pub registers: &'static [&'static str],
    pub symbols: fn(&str) -> bool,
    identifier_kind: Option<fn(&str) -> Option<TokenKind>>,
}

impl SyntaxSpec {
    pub const fn new(registers: &'static [&'static str], symbols: fn(&str) -> bool) -> Self {
        Self {
            registers,
            symbols,
            identifier_kind: None,
        }
    }

    pub const fn with_identifier_kind(mut self, classify: fn(&str) -> Option<TokenKind>) -> Self {
        self.identifier_kind = Some(classify);
        self
    }

    fn classify_identifier(&self, text: &str) -> TokenKind {
        if let Some(kind) = self.identifier_kind.and_then(|classify| classify(text)) {
            return kind;
        }

        if self
            .registers
            .iter()
            .any(|register| register.eq_ignore_ascii_case(text))
        {
            TokenKind::Register
        } else if (self.symbols)(text) {
            TokenKind::Symbol
        } else {
            TokenKind::Plain
        }
    }
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
enum Operand {
    #[regex(r"[ \t\r\n]+")]
    Whitespace,
    #[token("#")]
    Imm,
    #[regex(r"\$[0-9a-fA-F]+")]
    #[regex(r"0[xX][0-9a-fA-F]+")]
    Hex,
    #[regex(r"[0-9]+")]
    Dec,
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,
    #[regex(r"[,():\[\]{}!^+\-]")]
    Punct,
}

pub fn tokenize(text: &str, mnemonic: TokenKind, syntax: &SyntaxSpec) -> Vec<DisasmToken> {
    let mut tokens = Vec::with_capacity(8);

    let split = text
        .bytes()
        .position(|b| b.is_ascii_whitespace())
        .unwrap_or(text.len());
    if split > 0 {
        tokens.push(DisasmToken::new(&text[..split], mnemonic));
    }

    let operands = text[split..].trim_start();
    let mut spans: Vec<(Range<usize>, TokenKind)> = Vec::with_capacity(8);
    let mut lexer = Operand::lexer(operands);
    while let Some(result) = lexer.next() {
        let span = lexer.span();
        let kind = match result {
            Ok(Operand::Whitespace) => {
                if let Some((last, _)) = spans.last_mut() {
                    last.end = span.end;
                }
                continue;
            }
            Ok(Operand::Hex) => {
                let digits = span.start + if lexer.slice().starts_with('$') { 1 } else { 2 };
                spans.push((span.start..digits, TokenKind::HexPrefix));
                spans.push((digits..span.end, TokenKind::Number));
                continue;
            }
            Ok(Operand::Imm) => TokenKind::ImmPrefix,
            Ok(Operand::Dec) => TokenKind::Number,
            Ok(Operand::Ident) => syntax.classify_identifier(lexer.slice()),
            Ok(Operand::Punct) => TokenKind::Punctuation,
            Err(()) => TokenKind::Plain,
        };
        spans.push((span, kind));
    }

    tokens.extend(
        spans
            .into_iter()
            .map(|(span, kind)| DisasmToken::new(&operands[span], kind)),
    );
    tokens
}

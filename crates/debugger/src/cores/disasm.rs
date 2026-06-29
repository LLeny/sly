use cirsium::widgets::{DisasmToken, TokenKind};
use logos::Logos;

pub struct SyntaxSpec {
    pub registers: &'static [&'static str],
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

//! Title occurs in [definition][] and [label end][label_end].
//!
//! They’re formed with the following BNF:
//!
//! ```bnf
//! ; Restriction: no blank lines.
//! ; Restriction: markers must match (in case of `(` with `)`).
//! title ::= marker [  *( code - '\\' | '\\' [ marker ] ) ] marker
//! marker ::= '"' | '\'' | '('
//! ```
//!
//! Titles can be double quoted (`"a"`), single quoted (`'a'`), or
//! parenthesized (`(a)`).
//!
//! Titles can contain line endings and whitespace, but they are not allowed to
//! contain blank lines.
//! They are allowed to be blank themselves.
//!
//! The title is interpreted as the [string][] content type.
//! That means that [character escapes][character_escape] and
//! [character references][character_reference] are allowed.
//!
//! ## References
//!
//! *   [`micromark-factory-title/index.js` in `micromark`](https://github.com/micromark/micromark/blob/main/packages/micromark-factory-title/dev/index.js)
//!
//! [definition]: crate::construct::definition
//! [string]: crate::content::string
//! [character_escape]: crate::construct::character_escape
//! [character_reference]: crate::construct::character_reference
//! [label_end]: crate::construct::label_end

use super::partial_space_or_tab::{space_or_tab_eol_with_options, EolOptions};
use crate::subtokenize::link;
use crate::token::Token;
use crate::tokenizer::{ContentType, State, Tokenizer};

/// Configuration.
///
/// You must pass the token types in that are used.
#[derive(Debug)]
pub struct Options {
    /// Token for the whole title.
    pub title: Token,
    /// Token for the marker.
    pub marker: Token,
    /// Token for the string inside the quotes.
    pub string: Token,
}

/// State needed to parse titles.
#[derive(Debug)]
struct Info {
    /// Whether we’ve seen data.
    connect: bool,
    /// Closing marker.
    marker: u8,
    /// Configuration.
    options: Options,
}

/// Before a title.
///
/// ```markdown
/// > | "a"
///     ^
/// ```
pub fn start(tokenizer: &mut Tokenizer, options: Options) -> State {
    match tokenizer.current {
        Some(b'"' | b'\'' | b'(') => {
            let marker = tokenizer.current.unwrap();
            let info = Info {
                connect: false,
                marker: if marker == b'(' { b')' } else { marker },
                options,
            };
            tokenizer.enter(info.options.title.clone());
            tokenizer.enter(info.options.marker.clone());
            tokenizer.consume();
            tokenizer.exit(info.options.marker.clone());
            State::Fn(Box::new(|t| begin(t, info)))
        }
        _ => State::Nok,
    }
}

/// After the opening marker.
///
/// This is also used when at the closing marker.
///
/// ```markdown
/// > | "a"
///      ^
/// ```
fn begin(tokenizer: &mut Tokenizer, info: Info) -> State {
    match tokenizer.current {
        Some(b'"' | b'\'' | b')') if tokenizer.current.unwrap() == info.marker => {
            tokenizer.enter(info.options.marker.clone());
            tokenizer.consume();
            tokenizer.exit(info.options.marker.clone());
            tokenizer.exit(info.options.title);
            State::Ok
        }
        _ => {
            tokenizer.enter(info.options.string.clone());
            at_break(tokenizer, info)
        }
    }
}

/// At something, before something else.
///
/// ```markdown
/// > | "a"
///      ^
/// ```
fn at_break(tokenizer: &mut Tokenizer, mut info: Info) -> State {
    match tokenizer.current {
        None => State::Nok,
        Some(b'\n') => tokenizer.go(
            space_or_tab_eol_with_options(EolOptions {
                content_type: Some(ContentType::String),
                connect: info.connect,
            }),
            |t| {
                info.connect = true;
                at_break(t, info)
            },
        )(tokenizer),
        Some(b'"' | b'\'' | b')') if tokenizer.current.unwrap() == info.marker => {
            tokenizer.exit(info.options.string.clone());
            begin(tokenizer, info)
        }
        Some(_) => {
            tokenizer.enter_with_content(Token::Data, Some(ContentType::String));

            if info.connect {
                let index = tokenizer.events.len() - 1;
                link(&mut tokenizer.events, index);
            } else {
                info.connect = true;
            }

            title(tokenizer, info)
        }
    }
}

/// In title text.
///
/// ```markdown
/// > | "a"
///      ^
/// ```
fn title(tokenizer: &mut Tokenizer, info: Info) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            tokenizer.exit(Token::Data);
            at_break(tokenizer, info)
        }
        Some(b'"' | b'\'' | b')') if tokenizer.current.unwrap() == info.marker => {
            tokenizer.exit(Token::Data);
            at_break(tokenizer, info)
        }
        Some(byte) => {
            let func = if matches!(byte, b'\\') { escape } else { title };
            tokenizer.consume();
            State::Fn(Box::new(move |t| func(t, info)))
        }
    }
}

/// After `\`, in title text.
///
/// ```markdown
/// > | "a\*b"
///      ^
/// ```
fn escape(tokenizer: &mut Tokenizer, info: Info) -> State {
    match tokenizer.current {
        Some(b'"' | b'\'' | b')') => {
            tokenizer.consume();
            State::Fn(Box::new(|t| title(t, info)))
        }
        _ => title(tokenizer, info),
    }
}

use std::iter;
use std::str;

use items::{GSError, Item};

type CharStream<'a> = iter::Peekable<str::Chars<'a>>;

pub fn lex(input: &str) -> Result<Box<[Item]>, GSError> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(ch) = lex_item(&mut chars) {
        tokens.push(ch?);
    }

    Ok(tokens.into_boxed_slice())
}

fn lex_item(mut chars: &mut CharStream) -> Option<Result<Item, GSError>> {
    loop {
        let item = match chars.next() {
            Some('#') => {
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch == '\n' {
                        break;
                    }
                }

                continue;
            }

            Some('"') => {
                let mut string = String::new();
                loop {
                    match chars.next() {
                        Some('\\') => match chars.next() {
                            Some(ch) => string.push(ch),
                            None => {
                                return Some(Err(GSError::Parse(
                                    "invalid escape sequence".to_string(),
                                )));
                            }
                        },

                        Some('"') => break,
                        Some(ch) => string.push(ch),
                        None => {
                            return Some(Err(GSError::Parse(
                                "eof while scanning string literal".to_string(),
                            )));
                        }
                    }
                }

                let string = string.replace("\\\\", "\\").replace("\\\"", "\"");
                Item::Str(string)
            }

            Some('{') => {
                let mut block_items = Vec::new();
                loop {
                    match chars.peek() {
                        Some(&'}') => {
                            chars.next();
                            break;
                        }

                        // We must handle whitespace here else we can read
                        // `None` and skip final '}'
                        Some(&ch) if ch.is_whitespace() => {
                            chars.next();
                            continue;
                        }

                        // Handle eof/`None` on `lex_item` call
                        Some(_) | None => {
                            let item = match lex_item(&mut chars) {
                                Some(ch) => ch,
                                None => {
                                    return Some(Err(GSError::Parse(
                                        "eof while scanning for '}'".to_string(),
                                    )))
                                }
                            };

                            block_items.push(if let Ok(val) = item {
                                val
                            } else {
                                return Some(item);
                            });
                        }
                    }
                }

                Item::Block(block_items.into_boxed_slice())
            }

            Some(ch) if ch.is_digit(10) => {
                let mut num = String::new();
                num.push(ch);

                while let Some(&ch) = chars.peek() {
                    if ch.is_digit(10) {
                        num.push(ch);
                        chars.next();
                    } else {
                        break;
                    }
                }

                Item::Num(num.parse::<i64>().unwrap())
            }

            // If we encounter a '-' immediately followed by a number, this
            // is bound to the number instead of treated as an operator.
            Some(ch) if ch == '-' => match chars.peek() {
                Some(&ch) if ch.is_digit(10) => match lex_item(&mut chars) {
                    Some(Ok(Item::Num(x))) => Item::Num(-x),
                    Some(Err(e)) => return Some(Err(e)),
                    _ => unreachable!(),
                },

                _ => Item::Op('-'),
            },

            Some(ch) if ch.is_whitespace() => continue,
            Some(':') => {
                let mut string = String::new();
                // TODO: create a function for this
                loop {
                    match chars.peek() {
                        Some(ch) if ch.is_alphanumeric() || *ch == '_' => {
                            string.push(*ch);
                            chars.next();
                        }
                        Some(_) | None => break,
                    }
                }
                if string.is_empty() {
                    return Some(Err(GSError::Parse(
                        "empty variable name after :".to_string(),
                    )));
                }
                Item::Assign(string)
            }
            Some(ch @ '+') | Some(ch @ '-') | Some(ch @ '!') | Some(ch @ '@') | Some(ch @ '$')
            | Some(ch @ '*') | Some(ch @ '/') | Some(ch @ '%') | Some(ch @ '|')
            | Some(ch @ '&') | Some(ch @ '^') | Some(ch @ '\\') | Some(ch @ ';')
            | Some(ch @ '<') | Some(ch @ '>') | Some(ch @ '=') | Some(ch @ '.')
            | Some(ch @ '?') | Some(ch @ '(') | Some(ch @ ')') | Some(ch @ '[')
            | Some(ch @ ']') | Some(ch @ '~') | Some(ch @ '`') | Some(ch @ ',') => Item::Op(ch),
            Some(ch) => {
                let mut string = ch.to_string();
                // TODO: create a function for this
                loop {
                    match chars.next() {
                        Some(ch) if ch.is_alphanumeric() || ch == '_' => string.push(ch),
                        Some(_) | None => break,
                    }
                }
                Item::Var(string)
            }

            _ => return None,
        };

        return Some(Ok(item));
    }
}

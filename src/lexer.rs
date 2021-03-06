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

fn lex_variable(chars: &mut CharStream) -> String {
    // Match either a single symbol or a variable name
    match chars.peek() {
        Some('+') | Some('-') | Some('!') | Some('@') | Some('$') | Some('*') | Some('/')
        | Some('%') | Some('|') | Some('&') | Some('^') | Some('\\') | Some(';') | Some('<')
        | Some('>') | Some('=') | Some('.') | Some('?') | Some('(') | Some(')') | Some('[')
        | Some(']') | Some('~') | Some('`') | Some(',') => {
            return chars.next().unwrap().to_string();
        }
        Some(_) | None => (),
    }
    let mut string = String::new();
    loop {
        match chars.peek() {
            Some(ch) if ch.is_alphanumeric() || *ch == '_' => {
                string.push(*ch);
                chars.next();
            }
            Some(_) | None => break,
        }
    }
    string
}

fn lex_item(mut chars: &mut CharStream) -> Option<Result<Item, GSError>> {
    loop {
        let item = match chars.peek() {
            Some('#') => {
                chars.next();
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }

            Some('"') => {
                chars.next();
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
                chars.next();
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

            Some(&ch) if ch.is_digit(10) => {
                chars.next();
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
            Some(ch) if *ch == '-' => {
                chars.next();
                match chars.peek() {
                    Some(&ch) if ch.is_digit(10) => match lex_item(&mut chars) {
                        Some(Ok(Item::Num(x))) => Item::Num(-x),
                        Some(Err(e)) => return Some(Err(e)),
                        _ => unreachable!(),
                    },

                    _ => Var!("-"),
                }
            }

            Some(ch) if ch.is_whitespace() => {
                chars.next();
                continue;
            }

            Some(':') => {
                chars.next();
                let var = lex_variable(chars);
                if var.is_empty() {
                    return Some(Err(GSError::Parse(
                        "empty variable name after :".to_string(),
                    )));
                }
                Item::Assign(var)
            }

            Some(_) => {
                let var = lex_variable(chars);
                // variable could be empty?
                Item::Var(var)
            }

            _ => return None,
        };

        return Some(Ok(item));
    }
}

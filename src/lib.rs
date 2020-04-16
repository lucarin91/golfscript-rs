#![allow(dead_code)]

extern crate itertools;
extern crate rand;

use std::char;
use rand::distributions::{IndependentSample, Range};
use std::{fmt, str, iter};
use itertools::Itertools;
use std::collections::HashMap;

type CharStream<'a> = iter::Peekable<str::Chars<'a>>;

#[derive(Debug, PartialEq)]
pub enum GSError {
    Parse(String),
    Runtime(String)
}

type GSErr = Result<(), GSError>;

/// An `Item` can exist on the stack.
#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd)]
pub enum Item {
    Op(char),
    Var(String),
    Assign(String),
    Num(i64),
    Str(String),
    Array(Box<[Item]>),
    Block(Box<[Item]>),
}

/// Allow `to_string` conversion for `Item`'s
impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Item::Op(x) => write!(f, "{}", x),
            Item::Var(x) => write!(f, "{}", x),
            Item::Num(ref x) => write!(f, "{}", x),
            Item::Str(ref x) => write!(f, "\"{}\"", x.replace("\n", "\\n")),
            Item::Array(ref x) => {
                let _ = write!(f, "[")?;
                let _ = write!(f, "{}", x.iter().format_default(" "))?;
                write!(f, "]")
            }
            Item::Block(ref x) => {
                let _ = write!(f, "{{")?;
                let _ = write!(f, "{}", x.iter().format_default(" "))?;
                write!(f, "}}")
            }
            Item::Assign(_) => write!(f, "")
        }
    }
}

impl Item {
    /// Upcast the specified `Item` into an `Item::Array`
    ///
    /// Accepts: Num, Array
    ///
    /// ### Num
    /// Transforms into a single element array with the number.
    ///
    /// ### Array
    /// Nop
    fn upcast_to_array(self) -> Item {
        match self {
            x @ Item::Num(_) => Item::Array(vec![x].into_boxed_slice()),
            x @ Item::Array(_) => x,
            _ => panic!("upcast_to_array only accepts num, array")
        }
    }

    /// Upcast the specified `Item` into a `Item::Str`
    ///
    /// Accepts: Num, Array, String
    ///
    /// ### Num
    /// Parses the integer as a string. `34 => '34'`.
    ///
    /// ### Array
    /// Converts each element into a string. `Num` is treated as an
    /// ascii value prior to conversion.
    ///
    /// ### Str
    /// Nop
    fn upcast_to_string(self) -> Item {
        match self {
            Item::Num(val) => Item::Str(val.to_string()),
            Item::Array(items) => Item::Str(items.into_iter().map(|item| {
                if let Item::Num(val) = item {
                   char::from_u32(*val as u32).unwrap().to_string()
                } else {
                    // TODO: can the clone be removed?
                    if let Item::Str(val) = item.clone().upcast_to_string() {
                        val
                    } else {
                        panic!("upcast_to_string only accepts Num, Array, String")
                    }
                }
                }).join("")),
            x @ Item::Str(_) => x,
            _ => panic!("upcast_to_string only accepts Num, Array, String")
        }
    }

    /// Upcast the specified `Item` into a `Item::Block`
    ///
    /// Accepts: Num, Array, String, Block
    ///
    /// ### Num
    fn upcast_to_block(self) -> Item {
        match self {
            x @ Item::Num(_) => Item::Block(vec![x].into_boxed_slice()),
            Item::Array(items) => {
                let mut res: Vec<Item> = Vec::new();
                for item in items.into_iter() {
                    if let Item::Block(val) = item.clone().upcast_to_block() {
                        for i in val.into_iter() {
                            // TODO: can the clone be removed?
                            res.push(i.clone());
                        }
                    } else {
                        panic!("upcast_to_block only accepts Num, Array, String, Block")
                    }
                } 
                Item::Block(res.into_boxed_slice())
            },
            x @ Item::Str(_) => Item::Block(vec![x].into_boxed_slice()),
            x @ Item::Block(_) => x,
            _ => panic!("upcast_to_block only accepts Num, Array, String, Block")
        }
    }
}

// Coerce the specified items a similar type.
fn coerce((x, y): (Item, Item)) -> (Item, Item) {
    match (x, y) {
        (x, y @ Item::Block(_)) | (x @ Item::Block(_), y)
            => (x.upcast_to_block(), y.upcast_to_block()),

        (x, y @ Item::Str(_)) | (x @ Item::Str(_), y)
            => (x.upcast_to_string(), y.upcast_to_string()),
        
        (x, y @ Item::Array(_)) | (x @ Item::Array(_), y)
            => (x.upcast_to_array(), y.upcast_to_array()),

        (x, y) => (x, y)
    }
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
                        Some('\\') => {
                            match chars.next() {
                                Some(ch) => string.push(ch),
                                None => {
                                    return Some(Err(GSError::Parse(
                                        "invalid escape sequence".to_string()
                                    )));
                                }
                            }
                        }

                        Some('"') => break,
                        Some(ch) => string.push(ch),
                        None => {
                            return Some(Err(GSError::Parse(
                                "eof while scanning string literal".to_string()
                            )));
                        }
                    }
                }

                let string = string.replace("\\\\", "\\")
                                   .replace("\\\"", "\"");
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
                            continue
                        }

                        // Handle eof/`None` on `lex_item` call
                        Some(_) | None => {
                            let item = match lex_item(&mut chars) {
                                Some(ch) => ch,
                                None => {
                                    return Some(Err(GSError::Parse(
                                        "eof while scanning for '}'".to_string()
                                    )))
                                }
                            };

                            block_items.push(
                                if item.is_ok() {
                                    item.unwrap()
                                } else {
                                    return Some(item)
                                }
                            );
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
            Some(ch) if ch == '-' => {
                match chars.peek() {
                    Some(&ch) if ch.is_digit(10) => {
                        match lex_item(&mut chars) {
                            Some(Ok(Item::Num(x))) => Item::Num(-x),
                            Some(Err(e)) => return Some(Err(e)),
                            _ => unreachable!()
                        }
                    }

                    _ => Item::Op('-')
                }
            }

            Some(ch) if ch.is_whitespace() => continue,
            Some(':') => {
                let mut string = String::new();
                // TODO: create a function for this
                loop {
                    match chars.peek() {
                        Some(ch) if ch.is_alphanumeric() || *ch == '_' => {
                            string.push(*ch);
                            chars.next();
                        },
                        Some(_) | None => break
                    }
                }
                if string.is_empty() {
                    return Some(Err(GSError::Parse("empty variable name after :".to_string())))
                }
                Item::Assign(string)
            },
            Some(ch @ '+') | Some(ch @ '-') | Some(ch @ '!') | Some(ch @ '@') | Some(ch @ '$')
            | Some(ch @ '*') | Some(ch @ '/') | Some(ch @ '%') | Some(ch @ '|') | Some(ch @ '&')
            | Some(ch @ '^') | Some(ch @ '\\') | Some(ch @ ';') | Some(ch @ '<') | Some(ch @ '>')
            | Some(ch @ '=') | Some(ch @ '.') | Some(ch @ '?') | Some(ch @ '(') | Some(ch @ ')')
            | Some(ch @ '[') | Some(ch @ ']') | Some(ch @ '~') | Some(ch @ '`') | Some(ch @ ',')
                => Item::Op(ch),
            Some(ch) => {
                let mut string = ch.to_string();
                // TODO: create a function for this
                loop {
                    match chars.next() {
                        Some(ch) if ch.is_alphanumeric() || ch == '_' => string.push(ch),
                        Some(_) | None => break
                    }
                }
                Item::Var(string)
            }

            _ => return None
        };

        return Some(Ok(item))
    }
}

pub fn lex(input: &str) -> Result<Box<[Item]>, GSError> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(ch) = lex_item(&mut chars) {
        tokens.push(ch?);
    }

    Ok(tokens.into_boxed_slice())
}

#[derive(Debug)]
pub struct Interpreter {
    stack: Vec<Item>,

    /// Store all past stack markers
    marker_stack: Vec<usize>,

    variables: HashMap<String, Item>
}

impl Interpreter {
    pub fn new() -> Interpreter {
        Interpreter {
            stack: Vec::new(),
            marker_stack: Vec::new(),
            variables: HashMap::new()
        }
    }

    /// Push a value onto the stack
    ///
    /// # Panics
    /// panics if the value exceeds the length of a usize.
    fn push(&mut self, value: Item) {
        self.stack.push(value)
    }

    /// Pop a single value off the stack.
    fn pop(&mut self) -> Result<Item, GSError> {
        match self.stack.pop() {
            Some(value) => {
                // Resize all markers that are beyond the edge of the stack
                for marker in self.marker_stack.iter_mut() {
                    if *marker > self.stack.len() {
                        *marker = self.stack.len();
                    }
                }

                Ok(value)
            }

            None => Err(GSError::Runtime("stack underflow".to_string()))
        }
    }

    /// Pop the top two values off the stack.
    fn pop2(&mut self) -> Result<(Item, Item), GSError> {
        // Order of execution in ',' values is defined in rust.
        Ok((self.pop()?, self.pop()?))
    }

    /// Peek last element of the stack.
    fn peek(&mut self) -> Result<Item, GSError> {
        match self.stack.last() {
            Some(value) => return Ok(value.clone()),
            None => Err(GSError::Runtime("stack underflow".to_string()))
        }
    }
    
    fn add_variable(&mut self, name: String, value: Item) {
        self.variables.insert(name, value);
    }
    
    fn get_variable(&mut self, name: &str) -> Result<Item, GSError> {
        match self.variables.get(name) {
            Some(value) => Ok(value.clone()),
            None => Err(GSError::Runtime(format!("variable '{}' not founded", name)))
        }
    }

    /// Execute a string, returning the stack state after execution
    pub fn exec(&mut self, input: &str) -> Result<&[Item], GSError> {
        let items = lex(&input)?;
        self.exec_items(&items)
    }

    /// Execute a sequence of items, returning the stack state after execution
    pub fn exec_items(&mut self, items: &[Item])-> Result<&[Item], GSError> {
        for item in items.iter().cloned() {
            match item {
                // Can we restructure to allow for rebound variables?
                Item::Op('+') => self.add()?,
                Item::Op('-') => self.sub()?,
                Item::Op('!') => self.not()?,
                Item::Op('@') => self.at()?,
                Item::Op('$') => self.dollar()?,
                Item::Op('*') => self.mul()?,
                Item::Op('/') => self.div()?,
                Item::Op('%') => self.modulo()?,
                Item::Op('|') => self.or()?,
                Item::Op('&') => self.and()?,
                Item::Op('^') => self.xor()?,
                Item::Op('\\') => self.swap()?,
                Item::Op(';') => self.pop_discard()?,
                Item::Op('<') => self.lt()?,
                Item::Op('>') => self.gt()?,
                Item::Op('=') => self.eq()?,
                Item::Op('.') => self.dup()?,
                Item::Op('?') => self.qmark()?,
                Item::Op('(') => self.dec()?,
                Item::Op(')') => self.inc()?,
                Item::Op('[') => self.marker()?,
                Item::Op(']') => self.slice()?,
                Item::Op('~') => self.neg()?,
                Item::Op('`') => self.backtick()?,
                Item::Op(',') => self.array()?,
                Item::Assign(name) => self.assign(name)?,
                Item::Var(ref name) if "abs" == name.as_str() => self.builtin_abs()?,
                Item::Var(ref name) if "if" == name.as_str() => self.builtin_if()?,
                Item::Var(ref name) if "rand" == name.as_str() => self.builtin_rand()?,
                Item::Var(ref name) if "print" == name.as_str() => self.builtin_print()?,
                Item::Var(ref name) if "n" == name.as_str() => self.builtin_n()?,
                Item::Var(name) => self.exec_variable(name.as_str())?,
                x @ Item::Num(_) | x @ Item::Str(_) | x @ Item::Block(_) => {
                    self.push(x)
                }

                x @ _ => {
                    return Err(GSError::Runtime(
                            format!("invalid token encountered: {:?}", x))
                    );
                }
            }
        }
        // println!("   STACK:{:?} VARIABLE:{:?}", self.stack, self.variables);

        Ok(&self.stack)
    }

    fn assign(&mut self, name: String) -> GSErr {
        let item = self.peek()?;
        self.add_variable(name, item);
        Ok(())
    }
    
    fn exec_variable(&mut self, name: &str) -> GSErr {
        match self.get_variable(name) {
            Ok(value) => {
                if let Item::Block(ref items) = value {
                    self.exec_items(items)?;
                } else {
                    self.push(value)
                }
                Ok(())
            }
            Err(err) => Err(err)
        }
    }

    /// +
    fn add(&mut self) -> GSErr {
        match coerce(self.pop2()?) {
            (Item::Num(x), Item::Num(y)) => self.push(Item::Num(x + y)),

            (Item::Str(x), Item::Str(y)) => {
                self.push(Item::Str(y + &x))
            }

            (Item::Array(x), Item::Array(y)) => {
                self.push(Item::Array(
                    y.iter().chain(x.iter())
                            .cloned()
                            .collect_vec()
                            .into_boxed_slice()
                ));

            }

            (Item::Block(x), Item::Block(y)) => {
                self.push(Item::Block(
                    y.iter().chain(x.iter())
                            .cloned()
                            .collect_vec()
                            .into_boxed_slice()
                ));
            }

            _ => unimplemented!()
        }

        Ok(())
    }

    /// -
    fn sub(&mut self) -> GSErr {
        // Handle parsing of numbers which '-' is a unary operator
        // This should be done in the lexer, a number is negative if the
        // '-' symbol immediately precedes the value (special lexer case)
        match coerce(self.pop2()?) {
            (Item::Num(x), Item::Num(y)) => self.push(Item::Num(y - x)),
            _ => unimplemented!()
        }

        Ok(())
    }

    /// !
    fn not(&mut self) -> GSErr {
        match self.pop()? {
            Item::Num(x) => {
                self.push(Item::Num(if x == 0 { 1 } else { 0 }))
            }

            Item::Str(ref x) => {
                self.push(Item::Num(if x == "" { 1 } else { 0 }))
            }

            Item::Array(ref x) | Item::Block(ref x) => {
                self.push(Item::Num(if x.is_empty() { 1 } else { 0 }))
            }

            _ => unimplemented!()
        }

        Ok(())
    }

    /// @
    fn at(&mut self) -> GSErr {
        let (x, y) = self.pop2()?;
        let z = self.pop()?;
        self.push(y);
        self.push(x);
        self.push(z);

        Ok(())
    }

    /// $
    fn dollar(&mut self) -> GSErr {
        match self.pop()? {
            Item::Num(x) => {
                if x >= self.stack.len() as i64 || x < 0 {
                    return Err(GSError::Runtime(
                        "attempting to index beyond stack".to_string()
                    ));
                }

                let os = self.stack.len() - x as usize - 1;
                let value = self.stack[os].clone();
                self.push(value);
            }

            Item::Str(x) => {
                let mut buf: Vec<char> = x.chars().collect();
                buf.sort();
                self.push(Item::Str(buf.into_iter().collect()))
            },

            _ => unimplemented!()
        }

        Ok(())
    }

    /// *
    fn mul(&mut self) -> GSErr {
        match self.pop2()? {
            (Item::Num(x), Item::Num(y)) => self.push(Item::Num(x * y)),

            (Item::Num(y), Item::Str(x)) |
            (Item::Str(x), Item::Num(y)) => {
                if y < 0 {
                    return Err(GSError::Runtime(
                        "repeat string value is negative".to_string()
                    ));
                }
                self.push(Item::Str(iter::repeat(x).take(y as usize).collect()))
            },

            (Item::Num(y), Item::Array(x)) |
            (Item::Array(x), Item::Num(y)) => {
                if y < 0 {
                    return Err(GSError::Runtime(
                        "repeat string value is negative".to_string()
                    ));
                }

                self.push(Item::Array(x.iter().cloned()
                                              .cycle()
                                              .take(x.len() * y as usize)
                                              .collect_vec()
                                              .into_boxed_slice()
                ))
            }

            _ => unimplemented!()
        }
        Ok(())
    }

    /// /
    fn div(&mut self) -> GSErr {
        match self.pop2()? {
            (Item::Num(x), Item::Num(y)) => self.push(Item::Num(y / x)),

            _ => unimplemented!()
        }
        Ok(())
    }

    /// %
    fn modulo(&mut self) -> GSErr {
        match self.pop2()? {
            (Item::Num(x), Item::Num(y)) => self.push(Item::Num(y % x)),

            _ => unimplemented!()
        }
        Ok(())
    }

    /// ~
    fn neg(&mut self) -> GSErr {
        match self.pop()? {
            Item::Num(x) => self.push(Item::Num(!x)),

            Item::Array(x) => {
                // Error here
                for item in x.iter().cloned() {
                    self.push(item)
                }
            }

            Item::Str(ref x) => {
                match self.exec(x) {
                    Err(e) => return Err(GSError::Runtime(
                                  format!("invalid expression statement: {:?}", e)
                              )),
                    _ => ()
                }
            }

            Item::Block(ref x) => {
                match self.exec_items(x) {
                    Err(e) => return Err(GSError::Runtime(
                                  format!("invalid expression statement: {:?}", e)
                              )),
                    _ => ()
                }
            }

            _ => unimplemented!()
        }
        Ok(())
    }

    /// `
    fn backtick(&mut self) -> GSErr {
        let item = self.pop()?.to_string();
        self.push(Item::Str(item));
        Ok(())
    }

    /// |
    fn or(&mut self) -> GSErr {
        match coerce(self.pop2()?) {
            (Item::Num(x), Item::Num(y)) => self.push(Item::Num(x | y)),

            (Item::Array(x), Item::Array(y)) => {
                self.push(Item::Array(
                    x.iter().cloned()
                            .chain(y.iter().cloned())
                            .unique()
                            .collect_vec()
                            .into_boxed_slice()
                ));
            }

            _ => unimplemented!()
        }
        Ok(())
    }

    /// &
    fn and(&mut self) -> GSErr {
        match coerce(self.pop2()?) {
            (Item::Num(x), Item::Num(y)) => self.push(Item::Num(x & y)),

            (Item::Array(x), Item::Array(y)) => {
                // TODO: Incorrect value
                self.push(Item::Array(
                    x.iter().cloned()
                            .filter(|ref x| !y.contains(x))
                            .unique()
                            .collect_vec()
                            .into_boxed_slice()
                ));
            }

            _ => unimplemented!()
        }
        Ok(())
    }

    /// ^
    fn xor(&mut self) -> GSErr {
        match coerce(self.pop2()?) {
            (Item::Num(x), Item::Num(y)) => self.push(Item::Num(x ^ y)),

            _ => unimplemented!()
        }
        Ok(())
    }

    // \
    fn swap(&mut self) -> GSErr {
        let (x, y) = self.pop2()?;
        self.push(x);
        self.push(y);
        Ok(())
    }

    // ;
    fn pop_discard(&mut self) -> GSErr {
        self.pop()?;
        Ok(())
    }

    // <
    fn lt(&mut self) -> GSErr {
        match self.pop2()? {
            (Item::Num(y), Item::Num(x)) => {
                self.push(Item::Num(
                    if x < y { 1 } else { 0 }
                ));
            }

            (Item::Str(y), Item::Str(x)) => {
                self.push(Item::Num(
                    if x < y { 1 } else { 0 }
                ));
            }

            (Item::Num(x), Item::Array(y)) |
            (Item::Array(y), Item::Num(x)) => {
                self.push(Item::Array(
                    y.iter().cloned()
                            .take(x as usize)
                            .collect_vec()
                            .into_boxed_slice()
                ));
            }

            _ => unimplemented!()
        }

        Ok(())
    }

    // >
    fn gt(&mut self) -> GSErr {
        match self.pop2()? {
            (Item::Num(y), Item::Num(x)) => {
                self.push(Item::Num(
                    if x > y { 1 } else { 0 }
                ));
            }
            (Item::Str(y), Item::Str(x)) => {
                self.push(Item::Num(
                    if x > y { 1 } else { 0 }
                ));
            }

            // Dont implement str specifically, upcast to an array and
            // apply this way

            (Item::Num(x), Item::Array(y)) |
            (Item::Array(y), Item::Num(x)) => {
                self.push(Item::Array(
                    y.iter().cloned()
                            .skip(x as usize)
                            .collect_vec()
                            .into_boxed_slice()
                ));
            }

            _ => unimplemented!()
        }

        Ok(())
    }

    // =
    fn eq(&mut self) -> GSErr {
        match self.pop2()? {
            (Item::Num(x), Item::Num(y)) => {
                self.push(Item::Num(
                    if x == y { 1 } else { 0 }
                ));
            }
            (Item::Str(x), Item::Str(y)) => {
                self.push(Item::Num(
                    if x == y { 1 } else { 0 }
                ));
            }

            (Item::Num(x), Item::Array(y)) |
            (Item::Array(y), Item::Num(x)) => {
                let os = if x < 0 { y.len() as i64 + x } else { x };

                if 0 <= os && os < y.len() as i64 {
                    self.push(y[os as usize].clone());
                }
            }

            _ => unimplemented!()
        }

        Ok(())
    }

    // ,
    fn array(&mut self) -> GSErr {
        match self.pop()? {
            Item::Num(x) => {
                self.push(Item::Array((0..x).map(Item::Num)
                                            .collect_vec()
                                            .into_boxed_slice()
                ));
            },

            Item::Array(x) => {
                self.push(Item::Num(x.len() as i64));
            },

            _ => unimplemented!()
        }
        Ok(())
    }

    // .
    fn dup(&mut self) -> GSErr {
        let x = self.pop()?;
        self.push(x.clone());
        self.push(x);
        Ok(())
    }

    // ?
    fn qmark(&mut self) -> GSErr {
        match self.pop2()? {
            (Item::Num(y), Item::Num(x)) => {
                if y < 0 {
                    return Err(GSError::Runtime(
                        "cannot raise to negative power".to_string()
                    ));
                }

                // Handle overflow somehow (may have to use own power)
                self.push(Item::Num(x.pow(y as u32)))
            }

            (Item::Num(x), Item::Array(y)) |
            (Item::Array(y), Item::Num(x)) => {
                self.push(Item::Num(
                        y.iter().position(|v| v == &Item::Num(x))
                                .map_or_else(|| -1, |x| x as i64)
                ));
            }

            _ => unimplemented!()
        }

        Ok(())
    }

    // (
    fn dec(&mut self) -> GSErr {
        match self.pop()? {
            Item::Num(x) => self.push(Item::Num(x - 1)),

            Item::Array(x) => {
                if !x.is_empty() {
                    let mut buf = x.into_vec();
                    let cons = buf.remove(0);
                    self.push(Item::Array(buf.into_boxed_slice()));
                    self.push(cons);
                }
            }

            _ => unimplemented!()
        }

        Ok(())
    }

    // )
    fn inc(&mut self) -> GSErr {
        match self.pop()? {
            Item::Num(x) => self.push(Item::Num(x + 1)),

            Item::Array(x) => {
                if !x.is_empty() {
                    let mut buf = x.into_vec();
                    let uncons = buf.pop().unwrap();
                    self.push(Item::Array(buf.into_boxed_slice()));
                    self.push(uncons);
                }
            }

            _ => unimplemented!()
        }

        Ok(())
    }

    // [
    fn marker(&mut self) -> GSErr {
        self.marker_stack.push(self.stack.len());
        Ok(())
    }

    // ]
    fn slice(&mut self) -> GSErr {
        let offset = match self.marker_stack.pop() {
            Some(value) => value,
            None => return Err(GSError::Runtime("marker stack underflow".to_string()))
        };

        let array_items = self.stack.split_off(offset).into_boxed_slice();
        self.push(Item::Array(array_items));
        Ok(())
    }

    // abs
    fn builtin_abs(&mut self) -> GSErr {
        match self.pop()? {
            Item::Num(x) => self.push(Item::Num(x.abs())),
            x @ _ => return Err(GSError::Runtime(
                         format!("invalid type for `abs`: {:?}", x)
                     ))
        }
        Ok(())
    }

    // if
    fn builtin_if(&mut self) -> GSErr {
        self.not()?; // Evaluate if top of stack is not, note this requires
                     // a reverse condition check in the following.

        // TODO: consider block case
        match self.pop()? {
            Item::Num(x) => {
                let (a, b) = self.pop2()?;

                self.push(
                    if x == 0 {
                        a
                    } else if x == 1 {
                        b
                    } else {
                        panic!("expected 0 or 1 but found: {:?}", x)
                    }
                );
            },

            x @ _ => panic!("expected number but found: {:?}", x)
        }

        Ok(())
    }

    // rand
    fn builtin_rand(&mut self) -> GSErr {
        match self.pop()? {
            Item::Num(x) => {
                if x == 0 {
                    return Err(GSError::Runtime(
                        "invalid random range: [0, 0)".to_string()
                    ));
                }

                let range = if x < 0 {
                    Range::new(x, 0)
                } else {
                    Range::new(0, x)
                };

                let mut rng = rand::thread_rng();
                self.push(Item::Num(range.ind_sample(&mut rng)))
            }

            x @ _ => panic!("invalid type for `rand`: {:?}", x)
        }
        Ok(())
    }

    // print
    fn builtin_print(&mut self) -> GSErr {
        println!("{:?}", self.pop()?);
        Ok(())
    }

    // n (newline)
    fn builtin_n(&mut self) -> GSErr {
        self.push(Item::Str("\n".to_string()));
        Ok(())
    }
}

#[allow(unused_imports)]
#[cfg(test)]
mod tests {
    use super::*;
    use super::Item::*;

    // Helper macros for initializing items
    macro_rules! Array {
        ($x:expr) => {{ Array(Box::new($x)) }};
    }

    macro_rules! Str {
        ($x:expr) => {{ Str($x.to_string()) }};
    }

    macro_rules! Block {
        ($x:expr) => {{ Block(Box::new($x)) }};
    }

    fn eval_(input: &str) -> Result<Vec<Item>, GSError> {
        let mut it = Interpreter::new();
        it.exec(&input).map(|x| x.to_vec())
    }

    fn eval(input: &str) -> Vec<Item> {
        eval_(&input).unwrap()
    }


    // test~
    #[test]
    fn negate_num() {
        assert_eq!(eval("5~"), [Num(-6)])
    }

    #[test]
    fn negate_str() {
        assert_eq!(eval("\"1 2+\"~"), [Num(3)]);
    }

    #[test]
    fn negate_array() {
        assert_eq!(eval("[1 2 3]~"), [Num(1), Num(2), Num(3)]);
    }

    #[test]
    fn negate_block() {
        assert_eq!(eval("{1 2+}~"), [Num(3)]);
    }

    // test`
    #[test]
    fn backtick_num() {
        assert_eq!(eval("1`"), [Str!("1")]);
    }

    #[test]
    fn backtick_str() {
        assert_eq!(eval("\"1\"`"), [Str!("\"1\"")]);
    }

    #[test]
    fn backtick_array() {
        assert_eq!(eval("[1 [2] \"asdf\"]`"), [Str!("[1 [2] \"asdf\"]")]);
    }

    #[test]
    fn backtick_block() {
        assert_eq!(eval("{1}`"), [Str!("{1}")]);
    }

    // test!
    #[test]
    fn exclaim_num() {
        assert_eq!(eval("0!"), [Num(1)]);
        assert_eq!(eval("1!"), [Num(0)]);
    }

    #[test]
    fn exclaim_str() {
        assert_eq!(eval("\"\"!"), [Num(1)]);
        assert_eq!(eval("\"asdf\"!"), [Num(0)]);
    }

    #[test]
    fn exclaim_array() {
        assert_eq!(eval("[]!"), [Num(1)]);
        assert_eq!(eval("[1 4]!"), [Num(0)]);
    }

    #[test]
    fn exclaim_block() {
        assert_eq!(eval("{}!"), [Num(1)]);
        assert_eq!(eval("{5}!"), [Num(0)]);
    }

    // test@
    #[test]
    fn at() {
        assert_eq!(eval("1 2 3 4@"), [Num(1), Num(3), Num(4), Num(2)]);
    }

    // test#
    #[test]
    fn hash() {
        assert_eq!(eval("1 # Here is a comment"), [Num(1)]);
    }

    // test$
    #[test]
    fn dollar_num() {
        assert_eq!(eval("1 2 3 4 5 1$"), [Num(1), Num(2), Num(3), Num(4), Num(5), Num(4)]);
    }

    #[test]
    fn dollar_str() {
        assert_eq!(eval("\"asdf\"$"), [Str!("adfs")]);
    }

    // test+
    #[test]
    fn add_num() {
        assert_eq!(eval("5 7+"), [Num(12)]);
    }

    #[test]
    fn add_str() {
        assert_eq!(eval("\"a\"\"b\"+"), [Str!("ab")]);
    }

    #[test]
    fn add_array() {
        assert_eq!(eval("[1][2]+"), [Array!([Num(1), Num(2)])]);
    }

    #[test]
    fn add_block() {
        assert_eq!(eval("{1}{2-}+"), [Block!([Num(1), Num(2), Op('-')])]);
    }

    // test-
    #[test]
    fn sub_num() {
        assert_eq!(eval("-1"), [Num(-1)]);
        assert_eq!(eval("1 2-3+"), [Num(1), Num(-1)]);
        assert_eq!(eval("1 2 -3+"), [Num(1), Num(-1)]);
        assert_eq!(eval("1 2- 3+"), [Num(2)]);
    }

    fn sub_array() {
        assert_eq!(eval("[5 2 5 4 1 1][1 2]-"), [Num(5), Num(5), Num(4)]);
    }

    // test*
    #[test]
    fn mul_num() {
        assert_eq!(eval("2 4*"), [Num(8)]);
    }

    #[test]
    fn mul_num_str() {
        assert_eq!(eval("\"asdf\"3*"), [Str!("asdfasdfasdf")]);
        assert_eq!(eval("3\"asdf\"*"), [Str!("asdfasdfasdf")]);
    }

    #[test]
    fn mul_num_array() {
        assert_eq!(eval("[1 2]2*"), [Array!([Num(1), Num(2), Num(1), Num(2)])]);
        assert_eq!(eval("2[1 2]*"), [Array!([Num(1), Num(2), Num(1), Num(2)])]);
    }

    fn mul_join() {
    }

    fn mul_fold() {
    }

    // test/
    #[test]
    fn div_num() {
        assert_eq!(eval("7 3/"), [Num(2)]);
    }

    // test%
    #[test]
    fn mod_num() {
        assert_eq!(eval("7 3%"), [Num(1)]);
    }

    // test|
    #[test]
    fn or_num() {
        assert_eq!(eval("5 3|"), [Num(7)]);
    }

    // test&
    #[test]
    fn and_num() {
        assert_eq!(eval("5 3&"), [Num(1)]);
    }

    // test^
    #[test]
    fn xor_num() {
        assert_eq!(eval("5 3^"), [Num(6)]);
    }

    // test[]
    #[test]
    fn slice() {
        assert_eq!(eval("[1 2]"), [Array!([Num(1), Num(2)])]);
        assert_eq!(eval("1 2 [\\]"), [Array!([Num(2), Num(1)])]);
    }

    // test\
    #[test]
    fn swap() {
        assert_eq!(eval("1 2 3\\"), [Num(1), Num(3), Num(2)]);
    }

    // test;
    #[test]
    fn pop_discard() {
        assert_eq!(eval("1;"), []);
        assert_eq!(eval("2 1;"), [Num(2)]);
    }

    // test<
    #[test]
    fn lt_num() {
        assert_eq!(eval("3 4<"), [Num(1)]);
    }

    #[test]
    fn lt_str() {
        assert_eq!(eval("\"asdf\"\"asdg\"<"), [Num(1)]);
    }

    #[test]
    fn lt_num_array() {
        assert_eq!(eval("[1 2 3]2<"), [Array!([Num(1), Num(2)])]);
    }

    // test>
    #[test]
    fn gt_num() {
        assert_eq!(eval("3 4>"), [Num(0)]);
    }

    #[test]
    fn gt_str() {
        assert_eq!(eval("\"asdf\"\"asdg\">"), [Num(0)]);
    }

    #[test]
    fn gt_num_array() {
        assert_eq!(eval("[1 2 3]2>"), [Array!([Num(3)])]);
    }

    // test=
    #[test]
    fn eq_num() {
        assert_eq!(eval("3 4="), [Num(0)]);
    }

    #[test]
    fn eq_str() {
        assert_eq!(eval("\"asdf\"\"asdg\">"), [Num(0)]);
    }

    #[test]
    fn eq_num_array() {
        assert_eq!(eval("[1 2 3]2="), [Num(3)]);
        assert_eq!(eval("[1 2 3]-1="), [Num(3)]);
    }

    // test?
    #[test]
    fn qmark_num() {
        assert_eq!(eval("2 8?"), [Num(256)]);
    }

    #[test]
    fn qmark_num_array() {
        assert_eq!(eval("5 [4 3 5 1]?"), [Num(2)]);
    }

    // test(
    #[test]
    fn dec_num() {
        assert_eq!(eval("5("), [Num(4)]);
    }

    #[test]
    fn dec_array() {
        assert_eq!(eval("[1 2 3]("), [Array!([Num(2), Num(3)]), Num(1)]);
    }

    // test)
    #[test]
    fn inc_num() {
        assert_eq!(eval("5)"), [Num(6)]);
    }

    #[test]
    fn inc_array() {
        assert_eq!(eval("[1 2 3])"), [Array!([Num(1), Num(2)]), Num(3)]);
    }

    // test if
    #[test]
    fn builtin_if() {
        assert_eq!(eval("1 2 3if"), [Num(2)]);
    }

    // test abs
    #[test]
    fn builtin_abs() {
        assert_eq!(eval("-2abs"), [Num(2)]);
    }

    //test variable
    #[test]
    fn assignment(){
        assert_eq!(eval("\"hello\":str"), [Str!("hello")]);
        assert_eq!(eval("\"hello\":str;"), []);
        assert_eq!(eval("\"hello\":str;str"), [Str!("hello")]);
    }

    //test variable block
    #[test]
    fn assignment_block(){
        assert_eq!(eval("{-1*-}:plus;3 2 plus"), [Num(5)])
    }
}

// TODO: add coercion tests
extern crate itertools;
extern crate rand;

use std::collections::HashMap;
use std::str;

mod bultins;
mod items;
mod lexer;
pub use items::*;
use lexer::lex;
use Item::*;

#[derive(Debug, Default)]
pub struct Interpreter {
    stack: Vec<Item>,

    /// Store all past stack markers
    marker_stack: Vec<usize>,

    variables: HashMap<String, Item>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            stack: Vec::new(),
            marker_stack: Vec::new(),
            variables: HashMap::new(),
        }
    }

    /// Execute a string, returning the stack state after execution
    pub fn exec(&mut self, input: &str) -> Result<&[Item], GSError> {
        let items = lex(&input)?;
        self.exec_items(&items)
    }

    /// Execute a sequence of items, returning the stack state after execution
    pub fn exec_items(&mut self, items: &[Item]) -> Result<&[Item], GSError> {
        for item in items {
            match item {
                // Can we restructure to allow for rebound variables?
                Op('+') => self.add()?,
                Op('-') => self.sub()?,
                Op('!') => self.not()?,
                Op('@') => self.at()?,
                Op('$') => self.dollar()?,
                Op('*') => self.mul()?,
                Op('/') => self.div()?,
                Op('%') => self.modulo()?,
                Op('|') => self.or()?,
                Op('&') => self.and()?,
                Op('^') => self.xor()?,
                Op('\\') => self.swap()?,
                Op(';') => self.pop_discard()?,
                Op('<') => self.lt()?,
                Op('>') => self.gt()?,
                Op('=') => self.eq()?,
                Op('.') => self.dup()?,
                Op('?') => self.qmark()?,
                Op('(') => self.dec()?,
                Op(')') => self.inc()?,
                Op('[') => self.marker()?,
                Op(']') => self.slice()?,
                Op('~') => self.neg()?,
                Op('`') => self.backtick()?,
                Op(',') => self.array()?,
                Assign(name) => self.assign(name.clone())?,
                Var(name) if "abs" == name.as_str() => self.builtin_abs()?,
                Var(name) if "if" == name.as_str() => self.builtin_if()?,
                Var(name) if "rand" == name.as_str() => self.builtin_rand()?,
                Var(name) if "print" == name.as_str() => self.builtin_print()?,
                Var(name) if "n" == name.as_str() => self.builtin_n()?,
                Var(name) => self.exec_variable(name.as_str())?,
                x @ Num(_) | x @ Str(_) | x @ Block(_) => self.push(x.clone()),

                x => {
                    return Err(GSError::Runtime(format!(
                        "invalid token encountered: {:?}",
                        x
                    )));
                }
            }
        }
        // println!("   STACK:{:?} VARIABLE:{:?}", self.stack, self.variables);

        Ok(&self.stack)
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

            None => Err(GSError::Runtime("stack underflow".to_string())),
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
            Some(value) => Ok(value.clone()),
            None => Err(GSError::Runtime("stack underflow".to_string())),
        }
    }

    fn add_variable(&mut self, name: String, value: Item) {
        self.variables.insert(name, value);
    }
    fn get_variable(&mut self, name: &str) -> Result<Item, GSError> {
        match self.variables.get(name) {
            Some(value) => Ok(value.clone()),
            None => Err(GSError::Runtime(format!("variable '{}' not founded", name))),
        }
    }

    fn fun_call(&mut self, block: &[Item]) -> Result<Item, GSError> {
        self.exec_items(&block).unwrap();
        self.pop()
    }

    fn fun_call_with(&mut self, block: &[Item], val: Item) -> Result<Item, GSError> {
        self.push(val);
        self.fun_call(block)
    }
}

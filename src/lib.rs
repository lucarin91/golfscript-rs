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
            variables: Interpreter::get_default_var(),
        }
    }

    fn get_default_var() -> HashMap<String, Item> {
        let mut variables = HashMap::new();
        // Set predefined variables
        variables.insert(
            "and".to_string(),
            Block(Box::new([Num(1), Var!("$"), Var!("if")])),
        );
        variables.insert(
            "or".to_string(),
            Block(Box::new([Num(1), Var!("$"), Var!("\\"), Var!("if")])),
        );
        variables.insert(
            "xor".to_string(),
            Block(Box::new([
                Var!("\\"),
                Var!("!"),
                Var!("!"),
                Block(Box::new([Var!("!")])),
                Var!("*"),
            ])),
        );
        variables.insert("n".to_string(), Str("\n".to_string()));
        variables.insert(
            "puts".to_string(),
            Block(Box::new([
                Var("print".to_string()),
                Var("n".to_string()),
                Var("print".to_string()),
            ])),
        );
        variables.insert("p".to_string(), Block(Box::new([Var!("`"), Var!("puts")])));
        variables
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
                x @ Num(_) | x @ Str(_) | x @ Block(_) => self.push(x.clone()),
                Assign(name) => self.assign(name.clone())?,
                Var(name) if self.variables.contains_key(name) => {
                    self.exec_variable(name.as_str())?
                }
                Var(name) if "+" == name.as_str() => self.add()?,
                Var(name) if "-" == name.as_str() => self.sub()?,
                Var(name) if "!" == name.as_str() => self.not()?,
                Var(name) if "@" == name.as_str() => self.at()?,
                Var(name) if "$" == name.as_str() => self.dollar()?,
                Var(name) if "*" == name.as_str() => self.mul()?,
                Var(name) if "/" == name.as_str() => self.div()?,
                Var(name) if "%" == name.as_str() => self.modulo()?,
                Var(name) if "|" == name.as_str() => self.or()?,
                Var(name) if "&" == name.as_str() => self.and()?,
                Var(name) if "^" == name.as_str() => self.xor()?,
                Var(name) if "\\" == name.as_str() => self.swap()?,
                Var(name) if ";" == name.as_str() => self.pop_discard()?,
                Var(name) if "<" == name.as_str() => self.lt()?,
                Var(name) if ">" == name.as_str() => self.gt()?,
                Var(name) if "=" == name.as_str() => self.eq()?,
                Var(name) if "." == name.as_str() => self.dup()?,
                Var(name) if "?" == name.as_str() => self.qmark()?,
                Var(name) if "(" == name.as_str() => self.dec()?,
                Var(name) if ")" == name.as_str() => self.inc()?,
                Var(name) if "[" == name.as_str() => self.marker()?,
                Var(name) if "]" == name.as_str() => self.slice()?,
                Var(name) if "~" == name.as_str() => self.neg()?,
                Var(name) if "`" == name.as_str() => self.backtick()?,
                Var(name) if "," == name.as_str() => self.array()?,
                Var(name) if "abs" == name.as_str() => self.builtin_abs()?,
                Var(name) if "if" == name.as_str() => self.builtin_if()?,
                Var(name) if "rand" == name.as_str() => self.builtin_rand()?,
                Var(name) if "print" == name.as_str() => self.builtin_print()?,
                Var(name) if "zip" == name.as_str() => self.builtin_zip()?,
                Var(name) => {
                    return Err(GSError::Runtime(format!("variable '{}' not founded", name)))
                }
                x => {
                    return Err(GSError::Runtime(format!(
                        "invalid token encountered: {:?}",
                        x
                    )))
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

    fn fun_call(&mut self, block: &[Item]) -> Result<Vec<Item>, GSError> {
        let prev_size = self.stack.len();
        match self.exec_items(&block) {
            Ok(_) => Ok(self.stack.drain(prev_size - 1..).collect::<Vec<Item>>()),
            Err(err) => Err(err),
        }
    }

    fn fun_call_with(&mut self, block: &[Item], val: Item) -> Result<Vec<Item>, GSError> {
        self.push(val);
        self.fun_call(block)
    }
}

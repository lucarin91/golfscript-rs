extern crate itertools;
extern crate rand;

use itertools::Itertools;
use rand::distributions::{IndependentSample, Range};
use std::{char, iter, mem};

use items::{GSError, Item};
use Interpreter;
use Item::*;

type GSErr = Result<(), GSError>;

// Coerce the specified items a similar type.
fn coerce((x, y): (Item, Item)) -> (Item, Item) {
    match (x, y) {
        (x, y @ Block(_)) | (x @ Block(_), y) => (x.upcast_to_block(), y.upcast_to_block()),

        (x, y @ Str(_)) | (x @ Str(_), y) => (x.upcast_to_string(), y.upcast_to_string()),

        (x, y @ Array(_)) | (x @ Array(_), y) => (x.upcast_to_array(), y.upcast_to_array()),

        (x, y) => (x, y),
    }
}

impl Interpreter {
    /// +
    pub fn add(&mut self) -> GSErr {
        match coerce(self.pop2()?) {
            (Num(x), Num(y)) => self.push(Num(x + y)),

            (Str(x), Str(y)) => self.push(Str(y + &x)),

            (Array(x), Array(y)) => {
                let mut y = y.into_vec();
                y.extend(x.into_vec().into_iter());
                self.push(Array(y.into_boxed_slice()));
            }

            (Block(x), Block(y)) => {
                let mut y = y.into_vec();
                y.extend(x.into_vec().into_iter());
                self.push(Block(y.into_boxed_slice()));
            }

            _ => unimplemented!(),
        }

        Ok(())
    }

    /// -
    pub fn sub(&mut self) -> GSErr {
        // Handle parsing of numbers which '-' is a unary operator
        // This should be done in the lexer, a number is negative if the
        // '-' symbol immediately precedes the value (special lexer case)
        match coerce(self.pop2()?) {
            (Num(x), Num(y)) => self.push(Num(y - x)),
            (Array(x), Array(y)) => {
                self.push(Array(
                    y.into_vec()
                        .into_iter()
                        .filter(|el| !x.contains(el))
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }
            _ => unimplemented!(),
        }

        Ok(())
    }

    /// !
    pub fn not(&mut self) -> GSErr {
        match self.pop()? {
            Num(x) => self.push(Num(if x == 0 { 1 } else { 0 })),

            Str(ref x) => self.push(Num(if x == "" { 1 } else { 0 })),

            Array(ref x) | Block(ref x) => self.push(Num(if x.is_empty() { 1 } else { 0 })),

            _ => unimplemented!(),
        }

        Ok(())
    }

    /// @
    pub fn at(&mut self) -> GSErr {
        let (x, y) = self.pop2()?;
        let z = self.pop()?;
        self.push(y);
        self.push(x);
        self.push(z);

        Ok(())
    }

    /// $
    pub fn dollar(&mut self) -> GSErr {
        match self.pop()? {
            Num(x) => {
                if x >= self.stack.len() as i64 || x < 0 {
                    return Err(GSError::Runtime(
                        "attempting to index beyond stack".to_string(),
                    ));
                }

                let os = self.stack.len() - x as usize - 1;
                let value = self.stack[os].clone();
                self.push(value);
            }

            Str(x) => {
                let mut buf: Vec<char> = x.chars().collect();
                buf.sort();
                self.push(Str(buf.into_iter().collect()))
            }
            Array(mut items) => {
                items.sort();
                self.push(Array(items));
            }

            Block(ref block) => match self.pop()? {
                Array(mut items) => {
                    items.sort_by_cached_key(|a| self.fun_call_with(block, a.clone()).unwrap());
                    self.push(Array(items));
                }
                Str(val) => {
                    let mut buf: Vec<char> = val.chars().collect();
                    buf.sort_by_cached_key(|a| {
                        self.fun_call_with(block, Str(a.to_string())).unwrap()
                    });
                    self.push(Str(buf.into_iter().collect()))
                }
                _ => unimplemented!(),
            },
            _ => unimplemented!(),
        }

        Ok(())
    }

    /// *
    pub fn mul(&mut self) -> GSErr {
        match self.pop2()? {
            // multiplication
            (Num(x), Num(y)) => self.push(Num(x * y)),

            // repeat on Str Array and Block
            (Num(y), _) | (_, Num(y)) if y < 0 => {
                return Err(GSError::Runtime(
                    "repeat string value is negative".to_string(),
                ));
            }
            (Num(y), Str(x)) | (Str(x), Num(y)) => {
                self.push(Str(iter::repeat(x).take(y as usize).collect()))
            }
            (Num(y), Array(x)) | (Array(x), Num(y)) => {
                self.push(Array(
                    x.iter()
                        .cloned()
                        .cycle()
                        .take(x.len() * y as usize)
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }
            (Num(y), Block(x)) | (Block(x), Num(y)) => {
                for _ in 0..y {
                    self.exec_items(&x)?;
                }
            }

            // join on Array and Str
            (Array(x), Str(y)) | (Str(y), Array(x)) => {
                self.push(Str(x
                    .into_vec()
                    .into_iter()
                    .filter_map(|el| {
                        if let Str(val) = el.upcast_to_string() {
                            Some(val)
                        } else {
                            None
                        }
                    })
                    .join(y.as_str())));
            }
            (Array(y), Array(x)) => {
                let mut items: Vec<Item> = Vec::new();
                let mut x = x.into_vec().into_iter().peekable();
                while let Some(el) = x.next() {
                    match el {
                        Array(i) => items.extend(i.into_vec().into_iter()),
                        el => items.push(el),
                    }
                    if x.peek() != None {
                        items.extend_from_slice(&y);
                    }
                }
                self.push(Array(items.into_boxed_slice()));
            }
            (Str(y), Str(x)) => {
                self.push(Str(x.chars().join(&y)));
            }

            // fold on Array and Str
            (Block(y), Array(x)) | (Array(x), Block(y)) => {
                let x_len = x.len();
                for el in x.into_vec() {
                    self.push(el);
                }
                for _ in 1..x_len {
                    self.exec_items(&y)?;
                }
            }
            (Block(y), Str(x)) | (Str(x), Block(y)) => {
                for el in x.chars() {
                    self.push(Num(el as i64));
                }
                for _ in 1..x.len() {
                    self.exec_items(&y)?;
                }
            }

            _ => unimplemented!(),
        }
        Ok(())
    }

    /// /
    pub fn div(&mut self) -> GSErr {
        match self.pop2()? {
            (Num(y), Num(x)) => self.push(Num(x / y)),

            // split Array
            (Array(y), Array(x)) => {
                let mut yit = y.iter().cycle();
                let (mut v_nomatch, mut v_match, mut items) = (Vec::new(), Vec::new(), Vec::new());
                for el in x.into_vec() {
                    match yit.next() {
                        // save elements that match with split pattern
                        Some(i) if *i == el => v_match.push(el),
                        // save elements that do not match with split pattern
                        Some(i) if *i != el => {
                            v_nomatch.extend(mem::take(&mut v_match).into_iter());
                            v_nomatch.push(el);
                            yit = y.iter().cycle();
                        }
                        _ => (),
                    }
                    // split the array, all the pattern match
                    if v_match.len() == y.len() {
                        v_match.clear();
                        items.push(Array(mem::take(&mut v_nomatch).into_boxed_slice()));
                    }
                }
                // add remaining elements as last Array
                if !v_nomatch.is_empty() {
                    items.push(Array(v_nomatch.into_boxed_slice()));
                }
                self.push(Array(items.into_boxed_slice()));
            }

            // split Str
            (Str(y), Str(x)) => {
                self.push(Array(
                    x.split(y.as_str())
                        .map(|s| Str(s.to_owned()))
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }

            // chunk Array
            (Num(y), Array(x)) => {
                self.push(Array(
                    x.into_vec()
                        .into_iter()
                        .chunks_lazy(y as usize)
                        .into_iter()
                        .map(|c| Array(c.collect_vec().into_boxed_slice()))
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }

            // each Array
            (Block(y), Array(x)) => {
                for el in x.into_vec() {
                    self.push(el);
                    self.exec_items(&y)?;
                }
            }

            // unfold Block
            (Block(y), Block(x)) => {
                let mut items = Vec::new();
                loop {
                    self.dup()?;
                    let check = self.fun_call(&x)?;
                    match check.last().unwrap() {
                        Num(n) if *n != 0 => {
                            items.push(self.peek()?);
                            self.exec_items(&y)?;
                        }
                        Num(_) => {
                            self.pop()?;
                            break;
                        }
                        e => {
                            return Err(GSError::Runtime(format!(
                                "expected number but found: {:?}",
                                e
                            )))
                        }
                    }
                }
                self.push(Array(items.into_boxed_slice()));
            }

            _ => unimplemented!(),
        }
        Ok(())
    }

    /// %
    pub fn modulo(&mut self) -> GSErr {
        match self.pop2()? {
            (Num(y), Num(x)) => self.push(Num(x % y)),

            (Str(y), Str(x)) => {
                self.push(Array(
                    x.split(y.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| Str(s.to_owned()))
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }

            (Num(y), Array(x)) => {
                let mut items_vec = x
                    .into_vec()
                    .into_iter()
                    .enumerate()
                    .filter(|(i, _)| i % y.abs() as usize == 0)
                    .map(|(_, val)| val)
                    .collect_vec();
                if y < 0 {
                    items_vec.reverse();
                }
                self.push(Array(items_vec.into_boxed_slice()));
            }

            (Block(y), Array(x)) => {
                let items = x
                    .into_vec()
                    .into_iter()
                    .flat_map(|c| self.fun_call_with(&y, c).unwrap().into_iter())
                    .collect_vec()
                    .into_boxed_slice();
                self.push(Array(items));
            }

            _ => unimplemented!(),
        }
        Ok(())
    }

    /// ~
    pub fn neg(&mut self) -> GSErr {
        match self.pop()? {
            Num(x) => self.push(Num(!x)),

            Array(x) => {
                for item in x.into_vec() {
                    self.push(item)
                }
            }

            Str(ref x) => {
                if let Err(e) = self.exec(x) {
                    return Err(GSError::Runtime(format!(
                        "invalid expression statement: {:?}",
                        e
                    )));
                }
            }

            Block(ref x) => {
                if let Err(e) = self.exec_items(x) {
                    return Err(GSError::Runtime(format!(
                        "invalid expression statement: {:?}",
                        e
                    )));
                }
            }

            _ => unimplemented!(),
        }
        Ok(())
    }

    /// `
    pub fn backtick(&mut self) -> GSErr {
        let item = self.pop()?.to_string();
        self.push(Str(item));
        Ok(())
    }

    /// |
    pub fn or(&mut self) -> GSErr {
        match coerce(self.pop2()?) {
            (Num(x), Num(y)) => self.push(Num(x | y)),

            (Array(x), Array(y)) => {
                self.push(Array(
                    x.into_vec()
                        .into_iter()
                        .chain(y.into_vec().into_iter())
                        .unique()
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }

            _ => unimplemented!(),
        }
        Ok(())
    }

    /// &
    pub fn and(&mut self) -> GSErr {
        match coerce(self.pop2()?) {
            (Num(x), Num(y)) => self.push(Num(x & y)),

            (Array(x), Array(y)) => {
                // TODO: Incorrect value
                self.push(Array(
                    x.into_vec()
                        .into_iter()
                        .filter(|ref x| !y.contains(x))
                        .unique()
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }

            _ => unimplemented!(),
        }
        Ok(())
    }

    /// ^
    pub fn xor(&mut self) -> GSErr {
        match coerce(self.pop2()?) {
            (Num(x), Num(y)) => self.push(Num(x ^ y)),

            _ => unimplemented!(),
        }
        Ok(())
    }

    // \
    pub fn swap(&mut self) -> GSErr {
        let (x, y) = self.pop2()?;
        self.push(x);
        self.push(y);
        Ok(())
    }

    // ;
    pub fn pop_discard(&mut self) -> GSErr {
        self.pop()?;
        Ok(())
    }

    // <
    pub fn lt(&mut self) -> GSErr {
        match self.pop2()? {
            (Num(y), Num(x)) => {
                self.push(Num(if x < y { 1 } else { 0 }));
            }

            (Str(y), Str(x)) => {
                self.push(Num(if x < y { 1 } else { 0 }));
            }

            (Num(x), Array(y)) | (Array(y), Num(x)) => {
                self.push(Array(
                    y.into_vec()
                        .into_iter()
                        .take(x as usize)
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }

            _ => unimplemented!(),
        }

        Ok(())
    }

    // >
    pub fn gt(&mut self) -> GSErr {
        match self.pop2()? {
            (Num(y), Num(x)) => {
                self.push(Num(if x > y { 1 } else { 0 }));
            }
            (Str(y), Str(x)) => {
                self.push(Num(if x > y { 1 } else { 0 }));
            }

            // Dont implement str specifically, upcast to an array and
            // apply this way
            (Num(x), Array(y)) | (Array(y), Num(x)) => {
                self.push(Array(
                    y.into_vec()
                        .into_iter()
                        .skip(x as usize)
                        .collect_vec()
                        .into_boxed_slice(),
                ));
            }

            _ => unimplemented!(),
        }

        Ok(())
    }

    // =
    pub fn eq(&mut self) -> GSErr {
        match self.pop2()? {
            (Num(x), Num(y)) => {
                self.push(Num(if x == y { 1 } else { 0 }));
            }
            (Str(x), Str(y)) => {
                self.push(Num(if x == y { 1 } else { 0 }));
            }

            (Num(x), Array(y)) | (Array(y), Num(x)) => {
                let os = if x < 0 { y.len() as i64 + x } else { x };

                if 0 <= os && os < y.len() as i64 {
                    self.push(y[os as usize].clone());
                }
            }

            _ => unimplemented!(),
        }

        Ok(())
    }

    // ,
    pub fn array(&mut self) -> GSErr {
        match self.pop()? {
            Num(x) => {
                self.push(Array((0..x).map(Num).collect_vec().into_boxed_slice()));
            }

            Array(x) => {
                self.push(Num(x.len() as i64));
            }

            _ => unimplemented!(),
        }
        Ok(())
    }

    // .
    pub fn dup(&mut self) -> GSErr {
        let x = self.pop()?;
        self.push(x.clone());
        self.push(x);
        Ok(())
    }

    // ?
    pub fn qmark(&mut self) -> GSErr {
        match self.pop2()? {
            (Num(y), Num(x)) => {
                if y < 0 {
                    return Err(GSError::Runtime(
                        "cannot raise to negative power".to_string(),
                    ));
                }

                // Handle overflow somehow (may have to use own power)
                self.push(Num(x.pow(y as u32)))
            }

            (Num(x), Array(y)) | (Array(y), Num(x)) => {
                self.push(Num(y
                    .iter()
                    .position(|v| v == &Num(x))
                    .map_or_else(|| -1, |x| x as i64)));
            }

            _ => unimplemented!(),
        }

        Ok(())
    }

    // (
    pub fn dec(&mut self) -> GSErr {
        match self.pop()? {
            Num(x) => self.push(Num(x - 1)),

            Array(x) => {
                if !x.is_empty() {
                    let mut buf = x.into_vec();
                    let cons = buf.remove(0);
                    self.push(Array(buf.into_boxed_slice()));
                    self.push(cons);
                }
            }

            _ => unimplemented!(),
        }

        Ok(())
    }

    // )
    pub fn inc(&mut self) -> GSErr {
        match self.pop()? {
            Num(x) => self.push(Num(x + 1)),

            Array(x) => {
                if !x.is_empty() {
                    let mut buf = x.into_vec();
                    let uncons = buf.pop().unwrap();
                    self.push(Array(buf.into_boxed_slice()));
                    self.push(uncons);
                }
            }

            _ => unimplemented!(),
        }

        Ok(())
    }

    // [
    pub fn marker(&mut self) -> GSErr {
        self.marker_stack.push(self.stack.len());
        Ok(())
    }

    // ]
    pub fn slice(&mut self) -> GSErr {
        let offset = match self.marker_stack.pop() {
            Some(value) => value,
            None => return Err(GSError::Runtime("marker stack underflow".to_string())),
        };

        let array_items = self.stack.split_off(offset).into_boxed_slice();
        self.push(Array(array_items));
        Ok(())
    }

    // abs
    pub fn builtin_abs(&mut self) -> GSErr {
        match self.pop()? {
            Num(x) => self.push(Num(x.abs())),
            x => return Err(GSError::Runtime(format!("invalid type for `abs`: {:?}", x))),
        }
        Ok(())
    }

    // if
    pub fn builtin_if(&mut self) -> GSErr {
        self.not()?; // Evaluate if top of stack is not, note this requires
                     // a reverse condition check in the following.

        // TODO: consider block case
        match self.pop()? {
            Num(x) => {
                let (a, b) = self.pop2()?;

                self.push(if x == 0 {
                    a
                } else if x == 1 {
                    b
                } else {
                    panic!("expected 0 or 1 but found: {:?}", x)
                });
            }

            x => panic!("expected number but found: {:?}", x),
        }

        Ok(())
    }

    // rand
    pub fn builtin_rand(&mut self) -> GSErr {
        match self.pop()? {
            Num(x) => {
                if x == 0 {
                    return Err(GSError::Runtime("invalid random range: [0, 0)".to_string()));
                }

                let range = if x < 0 {
                    Range::new(x, 0)
                } else {
                    Range::new(0, x)
                };

                let mut rng = rand::thread_rng();
                self.push(Num(range.ind_sample(&mut rng)))
            }

            x => panic!("invalid type for `rand`: {:?}", x),
        }
        Ok(())
    }

    // print
    pub fn builtin_print(&mut self) -> GSErr {
        println!("{:?}", self.pop()?);
        Ok(())
    }

    // n (newline)
    pub fn builtin_n(&mut self) -> GSErr {
        self.push(Str("\n".to_string()));
        Ok(())
    }

    pub fn assign(&mut self, name: String) -> GSErr {
        let item = self.peek()?;
        self.add_variable(name, item);
        Ok(())
    }
    pub fn exec_variable(&mut self, name: &str) -> GSErr {
        match self.get_variable(name) {
            Ok(value) => {
                if let Block(ref items) = value {
                    self.exec_items(items)?;
                } else {
                    self.push(value)
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}

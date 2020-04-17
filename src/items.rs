extern crate itertools;

use std::cmp::Ordering;
use std::{char, fmt};

use itertools::Itertools;

#[derive(Debug, PartialEq)]
pub enum GSError {
    Parse(String),
    Runtime(String),
}

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
                write!(f, "[")?;
                write!(f, "{}", x.iter().format_default(" "))?;
                write!(f, "]")
            }
            Item::Block(ref x) => {
                write!(f, "{{")?;
                write!(f, "{}", x.iter().format_default(" "))?;
                write!(f, "}}")
            }
            Item::Assign(x) => write!(f, ":{}", x),
        }
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Item::Num(a), Item::Str(b)) => a.to_string().cmp(b),
            (Item::Str(a), Item::Num(b)) => a.cmp(&b.to_string()),

            (Item::Num(a), Item::Num(b)) => a.cmp(b),
            (Item::Str(a), Item::Str(b)) => a.cmp(b),

            _ => Ordering::Equal,
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
    pub fn upcast_to_array(self) -> Item {
        match self {
            x @ Item::Num(_) => Item::Array(vec![x].into_boxed_slice()),
            x @ Item::Array(_) => x,
            _ => panic!("upcast_to_array only accepts num, array"),
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
    pub fn upcast_to_string(self) -> Item {
        match self {
            Item::Num(val) => Item::Str(val.to_string()),
            Item::Array(items) => Item::Str(
                items
                    .iter()
                    .map(|item| {
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
                    })
                    .join(""),
            ),
            x @ Item::Str(_) => x,
            _ => panic!("upcast_to_string only accepts Num, Array, String"),
        }
    }

    /// Upcast the specified `Item` into a `Item::Block`
    ///
    /// Accepts: Num, Array, String, Block
    ///
    /// ### Num
    pub fn upcast_to_block(self) -> Item {
        match self {
            x @ Item::Num(_) => Item::Block(vec![x].into_boxed_slice()),
            Item::Array(items) => {
                let mut res: Vec<Item> = Vec::new();
                for item in items.iter() {
                    if let Item::Block(val) = item.clone().upcast_to_block() {
                        for i in val.iter() {
                            // TODO: can the clone be removed?
                            res.push(i.clone());
                        }
                    } else {
                        panic!("upcast_to_block only accepts Num, Array, String, Block")
                    }
                }
                Item::Block(res.into_boxed_slice())
            }
            x @ Item::Str(_) => Item::Block(vec![x].into_boxed_slice()),
            x @ Item::Block(_) => x,
            _ => panic!("upcast_to_block only accepts Num, Array, String, Block"),
        }
    }
}

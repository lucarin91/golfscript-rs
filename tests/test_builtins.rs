#![allow(dead_code)]

extern crate golfscript;

use golfscript::{GSError, Interpreter, Item};

use Item::*;

// Helper macros for initializing items
macro_rules! Array {
    ($x:expr) => {{
        Array(Box::new($x))
    }};
}

macro_rules! Str {
    ($x:expr) => {{
        Str($x.to_string())
    }};
}

macro_rules! Block {
    ($x:expr) => {{
        Block(Box::new($x))
    }};
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
fn negate_block() {
    assert_eq!(eval("{1 2+}~"), [Num(3)]);
}

#[test]
fn negate_array() {
    assert_eq!(eval("[1 2 3]~"), [Num(1), Num(2), Num(3)]);
}

// test`
#[test]
fn backtick_num() {
    assert_eq!(eval("1`"), [Str!("1")]);
}

#[test]
fn backtick_array() {
    assert_eq!(eval("[1 [2] \"asdf\"]`"), [Str!("[1 [2] \"asdf\"]")]);
}

#[test]
fn backtick_str() {
    assert_eq!(eval("\"1\"`"), [Str!("\"1\"")]);
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
fn exclaim_array() {
    assert_eq!(eval("[]!"), [Num(1)]);
    assert_eq!(eval("[1 4]!"), [Num(0)]);
}

#[test]
fn exclaim_str() {
    assert_eq!(eval("\"\"!"), [Num(1)]);
    assert_eq!(eval("\"asdf\"!"), [Num(0)]);
}

#[test]
fn exclaim_block() {
    assert_eq!(eval("{}!"), [Num(1)]);
    assert_eq!(eval("{5}!"), [Num(0)]);
}

// test@
#[test]
fn at() {
    assert_eq!(eval("1 2 3 4 @"), [Num(1), Num(3), Num(4), Num(2)]);
}

// test#
#[test]
fn hash() {
    assert_eq!(eval("1 # Here is a comment"), [Num(1)]);
}

// test$
#[test]
fn dollar_num() {
    assert_eq!(
        eval("1 2 3 4 5 1$"),
        [Num(1), Num(2), Num(3), Num(4), Num(5), Num(4)]
    );
}

#[test]
fn dollar_str() {
    assert_eq!(eval("\"asdf\"$"), [Str!("adfs")]);
}

#[test]
fn dollar_array() {
    assert_eq!(
        eval("[5 4 3 1 2]$"),
        [Array!([Num(1), Num(2), Num(3), Num(4), Num(5)])]
    );
    assert_eq!(
        eval("[\"ccc\" \"bbb\" \"aaa\"]$"),
        [Array!([Str!("aaa"), Str!("bbb"), Str!("ccc")])]
    );
}

#[test]
fn dollar_block() {
    assert_eq!(
        eval("[5 4 3 1 2]{-1*}$"),
        [Array!([Num(5), Num(4), Num(3), Num(2), Num(1)])]
    );
    assert_eq!(eval("\"asdf\"{\"\"+}$"), [Str!("adfs")]);
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

#[test]
fn add_coercion() {
    // to block
    assert_eq!(eval("\"a\"{2}+"), [Block!([Str!("a"), Num(2)])]);
    assert_eq!(eval("[1 2]{2}+"), [Block!([Num(1), Num(2), Num(2)])]);
    assert_eq!(eval("1{2}+"), [Block!([Num(1), Num(2)])]);

    // to string
    assert_eq!(eval("[50]\"b\"+"), [Str!("2b")]);
    assert_eq!(eval("1\"b\"+"), [Str!("1b")]);

    // to array
    assert_eq!(eval("1[2]+"), [Array!([Num(1), Num(2)])]);
}

// test-
#[test]
fn sub_num() {
    assert_eq!(eval("-1"), [Num(-1)]);
    assert_eq!(eval("1 2-3+"), [Num(1), Num(-1)]);
    assert_eq!(eval("1 2 -3+"), [Num(1), Num(-1)]);
    assert_eq!(eval("1 2- 3+"), [Num(2)]);
}

#[test]
fn sub_array() {
    assert_eq!(
        eval("[5 2 5 4 1 1][1 2]-"),
        [Array!([Num(5), Num(5), Num(4)])]
    );
}

#[test]
fn sub_coercion() {
    assert_eq!(eval("[1 2 3]2-"), [Array!([Num(1), Num(3)])]);
}

// test*
#[test]
fn mul_num() {
    assert_eq!(eval("2 4*"), [Num(8)]);
    assert_eq!(eval("4 2*"), [Num(8)]);
}

#[test]
fn mul_num_block() {
    assert_eq!(eval("2 {2*} 5*"), [Num(64)]);
    assert_eq!(eval("2 5 {2*}*"), [Num(64)]);
}

#[test]
fn mul_num_array() {
    assert_eq!(eval("[1 2] 2*"), [Array!([Num(1), Num(2), Num(1), Num(2)])]);
    assert_eq!(eval("2 [1 2]*"), [Array!([Num(1), Num(2), Num(1), Num(2)])]);
}

#[test]
fn mul_num_str() {
    assert_eq!(eval("\"asdf\"3*"), [Str!("asdfasdfasdf")]);
    assert_eq!(eval("3\"asdf\"*"), [Str!("asdfasdfasdf")]);
}

#[test]
fn mul_join_array_str() {
    assert_eq!(eval("[1 2]\",\"*"), [Str!("1,2")]);
    assert_eq!(eval("\",\"[1 2]*"), [Str!("1,2")]);
    assert_eq!(
        eval("[1 [2] [3 [4 [5]]]]\"-\"*"),
        [Str!("1-\u{2}-\u{3}\u{4}\u{5}")]
    );
    assert_eq!(
        eval("\"-\"[1 [2] [3 [4 [5]]]]*"),
        [Str!("1-\u{2}-\u{3}\u{4}\u{5}")]
    );
}

#[test]
fn mul_join_array_array() {
    assert_eq!(eval("[1 2][4]*"), [Array!([Num(1), Num(4), Num(2)])]);
    assert_eq!(
        eval("[1 [2] [3 [4 [5]]]] [6 7]*"),
        [Array!([
            Num(1),
            Num(6),
            Num(7),
            Num(2),
            Num(6),
            Num(7),
            Num(3),
            Array!([Num(4), Array!([Num(5)])])
        ])]
    );
}

#[test]
fn mul_join_str_str() {
    assert_eq!(eval("\"asdf\"\" \"*"), [Str!("a s d f")]);
}

#[test]
fn mul_fold_array() {
    assert_eq!(eval("[1 2 3 4]{+}*"), [Num(10)]);
    assert_eq!(eval("{+}[1 2 3 4]*"), [Num(10)]);
}

#[test]
fn mul_fold_str() {
    assert_eq!(eval("\"asdf\"{+}*"), [Num(414)]);
    assert_eq!(eval("{+}\"asdf\"*"), [Num(414)]);
}

// test/
#[test]
fn div_num() {
    assert_eq!(eval("7 3/"), [Num(2)]);
}

#[test]
fn div_split_array() {
    assert_eq!(
        eval("[1 2 3 4 2 3 5][2 3]/"),
        [Array!([
            Array!([Num(1)]),
            Array!([Num(4)]),
            Array!([Num(5)])
        ])]
    );
}

#[test]
fn div_split_str() {
    assert_eq!(
        eval("\"a s d f\"\" \"/"),
        [Array!([Str!("a"), Str!("s"), Str!("d"), Str!("f")])]
    );
}

#[test]
fn div_chunk() {
    assert_eq!(
        eval("[1 2 3 4 5]2/"),
        [Array!([
            Array!([Num(1), Num(2)]),
            Array!([Num(3), Num(4)]),
            Array!([Num(5)])
        ])]
    );
}

#[test]
fn div_unfold() {
    assert_eq!(
        eval("0 1 {10<}{.@+}/"),
        [
            Num(8),
            Array!([Num(1), Num(1), Num(2), Num(3), Num(5), Num(8)])
        ]
    );
}

#[test]
fn div_each() {
    assert_eq!(eval("[1 2 3]{1+}/"), [Array!([Num(2), Num(3), Num(4)])]);
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
fn assignment() {
    assert_eq!(eval("\"hello\":str"), [Str!("hello")]);
    assert_eq!(eval("\"hello\":str;"), []);
    assert_eq!(eval("\"hello\":str;str"), [Str!("hello")]);
}

//test variable block
#[test]
fn assignment_block() {
    assert_eq!(eval("{-1*-}:plus;3 2 plus"), [Num(5)])
}

// TODO: add coercion tests

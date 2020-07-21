//! This module defines and includes utilities for dealing with the intermediate CPS
//! representation of Rust used by Liquid Rust.

use rustc_span::Symbol;

// TODO: Where should our Box<>es be?

/// Each function in MIR is translated to a CpsFn
#[derive(Debug)]
pub struct CpsFn {
    pub name: Symbol,
    pub args: Vec<Tydent>,
    pub cont: Symbol,
    pub ret: Tydent,
    pub body: Box<Body>,
}

/// A Local is an identifier for some local variable (a fn arg, a let-bound
/// variable, or a letcont-bound continuation)
/// For now, these are symbols, but we could theoretically just use u32s
/// (since the name of the variables doesn't really matter)
pub type Local = Symbol;

/// A Tydent is a Reft with an associated identifier.
#[derive(Debug)]
pub struct Tydent {
    pub ident: Local,
    pub reft: Reft,
}

/// A Literal is a boolean or integer literal.
#[derive(Debug)]
pub enum Literal {
    Bool(bool),
    Int(i128),
}

/// A Projection is just a number.
pub type Projection = u32;

/// Paths are local variables with some projections into them.
#[derive(Debug)]
pub struct Path {
    pub ident: Local,
    pub projs: Vec<Projection>,
}

/// An Operand is either a path or a literal.
#[derive(Debug)]
pub enum Operand {
    Path(Path),
    Lit(Literal),
}

/// An RValue is an operand or some operations on them.
#[derive(Debug)]
pub enum RValue {
    Op(Operand),
    Binary(RBinOp, Operand, Operand),
}

/// BinOpKind is a binary operation on Operands.
#[derive(Debug)]
pub enum RBinOp {
    CheckedAdd,
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

/// A Body is (a part of) a function body.
#[derive(Debug)]
pub enum Body {
    Let(Local, RValue, Box<Body>),
    Cont(Local, Vec<Tydent>, Box<Body>, Box<Body>),
    Ite(Path, Box<Body>, Box<Body>),
    Call(Local, Vec<Path>, Local),
    Jump(Local, Vec<Path>),
    Abort,
}
    

/// A BasicType is a primitive type in the CPS IR; there are bools and ints.
#[derive(Debug)]
pub enum BasicType {
    Bool,
    Int(IntTy),
}

// An IntTy is a width and signedness for an int.
#[derive(Debug)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
}

#[derive(Debug)]
pub enum Reft {
    Fn { args: Vec<Tydent>, ret: Box<Reft> },
    Reft { ty: BasicType, pred: Pred },
    Proj(Vec<Reft>),
}

#[derive(Debug)]
pub enum Pred {
    Op(Operand),
    Binary(PredBinOp, Operand, Operand),
}

#[derive(Debug)]
pub enum PredBinOp {
    Add,
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

        

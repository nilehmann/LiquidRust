use std::fmt;

use super::BaseTy;
use crate::names::{Field, Location};

#[derive(Clone)]
pub enum Pred<S = usize> {
    Constant(Constant),
    Place(Place<S>),
    BinaryOp(BinOp, Box<Pred<S>>, Box<Pred<S>>),
    UnaryOp(UnOp, Box<Pred<S>>),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Constant {
    Bool(bool),
    Int(u128),
    Unit,
}

impl Constant {
    pub fn base_ty(&self) -> BaseTy {
        match self {
            Constant::Bool(_) => BaseTy::Bool,
            Constant::Int(_) => BaseTy::Int,
            Constant::Unit => BaseTy::Unit,
        }
    }
}

impl fmt::Display for Constant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constant::Bool(b) => write!(f, "{}", b),
            Constant::Int(n) => write!(f, "{}", n),
            Constant::Unit => write!(f, "()"),
        }
    }
}

#[derive(Eq, PartialEq, Clone, Hash)]
pub struct Place<S = usize> {
    pub base: Var<S>,
    pub projs: Vec<usize>,
}

impl<S: Copy> Place<S> {
    pub fn extend_path(&self, n: usize) -> Self {
        let mut projs = self.projs.clone();
        projs.push(n);
        Place {
            base: self.base,
            projs,
        }
    }
}

impl<T, S> From<T> for Place<S>
where
    T: Into<Var<S>>,
{
    fn from(base: T) -> Self {
        Place {
            base: base.into(),
            projs: Vec::new(),
        }
    }
}

#[derive(PartialEq, Eq, Copy, Clone, Debug, Hash)]
pub enum Var<S = usize> {
    Nu,
    Location(Location<S>),
    Field(Field<S>),
}

impl<S> From<Location<S>> for Var<S> {
    fn from(v: Location<S>) -> Self {
        Var::Location(v)
    }
}

impl<S> From<Field<S>> for Var<S> {
    fn from(v: Field<S>) -> Self {
        Var::Field(v)
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Var::Nu => write!(f, "v"),
            Var::Location(l) => write!(f, "l${}", l.inner()),
            Var::Field(fld) => write!(f, "f${}", fld.inner()),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
    Iff,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Lt => write!(f, "<"),
            BinOp::Le => write!(f, "<="),
            BinOp::Eq => write!(f, "="),
            BinOp::Ge => write!(f, ">="),
            BinOp::Gt => write!(f, ">"),
            BinOp::Iff => write!(f, "<=>"),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum UnOp {
    Not,
}

impl std::fmt::Display for UnOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnOp::Not => write!(f, "!"),
        }
    }
}

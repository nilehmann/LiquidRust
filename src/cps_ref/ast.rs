use std::fmt::Debug;

pub use rustc_span::Symbol;

/// A function definition
#[derive(Debug)]
pub struct FnDef<'lr> {
    /// Name of the function.
    pub name: Symbol,
    /// The input heap.
    pub heap: Heap<'lr>,
    /// Formal arguments of the function. These are always owned references.
    pub args: Vec<(Local, OwnRef)>,
    /// The return continuation.
    pub ret: Symbol,
    /// The output heap. This is right now only used to add refinements for the returned
    /// reference but it should be extended to capture the state of the output heap.
    pub out_heap: Heap<'lr>,
    /// Returned owned reference.
    pub out_ty: OwnRef,
    /// Body of the function.
    pub body: Box<FnBody<'lr>>,
}

#[derive(Debug)]
pub struct ContDef<'lr> {
    /// Name of the continuation.
    pub name: Symbol,
    /// Heap required to call the continuation.
    pub heap: Heap<'lr>,
    /// Environment required to call the continuation.
    pub env: Env,
    /// Additional parameters for the continuation.
    pub params: Vec<(Local, OwnRef)>,
    /// The body of the continuation.
    pub body: Box<FnBody<'lr>>,
}

/// Function body in cps.
#[derive(Debug)]
pub enum FnBody<'lr> {
    /// A continuation definition.
    LetCont {
        /// Continuation definition.
        def: ContDef<'lr>,
        /// The rest of the function body.
        rest: Box<FnBody<'lr>>,
    },

    /// Evaluates either the then or else branch depending on the value of the discriminant
    Ite {
        /// The discriminant value being tested.
        discr: Place,
        /// The branch to execute if the discriminant is true.
        then: Box<FnBody<'lr>>,
        /// The branch to execute if the discriminant is false.
        else_: Box<FnBody<'lr>>,
    },

    /// Function call
    Call {
        /// Name of the function to be called.
        func: Place,
        /// Arguments the function is called with. These are owned by the callee, which is free
        /// to modify them, i.e., arguments are always moved.
        args: Vec<Local>,
        /// The return continuation.
        ret: Symbol,
    },

    /// Jump to a continuation
    Jump {
        /// The target continuation.
        target: Symbol,
        /// Additional arguments to call the continuation with.
        args: Vec<Local>,
    },

    /// Sequencing of a statement and the rest of the function body
    Seq(Statement, Box<FnBody<'lr>>),

    /// Aborts the execution of the program
    Abort,
}

/// An statement
#[derive(Debug)]
pub enum Statement {
    /// Allocates a block of memory and binds the result to a local.
    /// The type layout is needed for the type system to know the recursive structure
    /// of the type a priori.
    Let(Local, TypeLayout),
    /// Either moves or copies the rvalue to a place.
    Assign(Place, Rvalue),
    Drop(Local),
}

/// An rvalue appears at the right hand side of an assignment.
#[derive(Debug)]
pub enum Rvalue {
    Use(Operand),
    Ref(BorrowKind, Place),
    BinaryOp(BinOp, Operand, Operand),
    CheckedBinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
}

/// A path to a value
#[derive(Eq, PartialEq, Hash, Clone)]
pub struct Place {
    pub local: Local,
    pub projection: Vec<Projection>,
}

pub struct Path(pub Vec<u32>);

impl Path {
    fn empty() -> Self {
        Path(vec![])
    }

    fn append(&self, i: u32) -> Self {
        let mut v = self.0.clone();
        v.push(i);
        Path(v)
    }
}

impl Place {
    pub fn new(local: Local, projection: Vec<Projection>) -> Self {
        Place { local, projection }
    }

    pub fn extend<'a, I>(&self, rhs: I) -> Place
    where
        I: IntoIterator<Item = &'a Projection>,
    {
        Place {
            local: self.local,
            projection: self
                .projection
                .iter()
                .copied()
                .chain(rhs.into_iter().copied())
                .collect(),
        }
    }

    pub fn overlaps(&self, rhs: &Place) -> bool {
        if self.local != rhs.local {
            return false;
        }
        for (&p1, &p2) in self.projection.iter().zip(&rhs.projection) {
            if p1 != p2 {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum Projection {
    Field(u32),
    Deref,
}

impl From<Local> for Place {
    fn from(local: Local) -> Self {
        Place {
            local,
            projection: vec![],
        }
    }
}

/// These are values that can appear inside an rvalue. They are intentionally limited to prevent
/// rvalues from being nested in one another.
#[derive(Debug)]
pub enum Operand {
    /// A place dereference *p. This may move or copy depending on the type of p.
    Deref(Place),
    /// A constant value
    Constant(Constant),
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum Constant {
    Bool(bool),
    Int(u128),
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum UnOp {
    Not,
}

/// The heap is a mapping between location and types.
pub type Heap<'lr> = Vec<(Location, Ty<'lr>)>;

/// A location into a block of memory in the heap.
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct Location(pub Symbol);

/// An environment maps locals to owned references.
pub type Env = Vec<(Local, OwnRef)>;

/// A Local is an identifier to some local variable introduced with a let
/// statement.
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct Local(pub Symbol);

/// An owned reference to a location. This is used for arguments and return types.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct OwnRef(pub Location);

pub type Ty<'lr> = &'lr TyS<'lr>;

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct Region(Vec<Place>);

impl Region {
    pub fn new(place: Place) -> Self {
        Self(vec![place])
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn subset_of(&self, rhs: &Region) -> bool {
        for place in &self.0 {
            if rhs.iter().any(|p| place == p) {
                return true;
            }
        }
        false
    }

    pub fn iter(&self) -> impl Iterator<Item = &Place> + '_ {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a Region {
    type Item = &'a Place;

    type IntoIter = std::slice::Iter<'a, Place>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl From<Vec<Place>> for Region {
    fn from(v: Vec<Place>) -> Self {
        Self(v)
    }
}

impl std::ops::Index<usize> for Region {
    type Output = Place;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, PartialOrd)]
pub enum BorrowKind {
    Shared,
    Mut,
}

impl BorrowKind {
    pub fn is_mut(&self) -> bool {
        match self {
            BorrowKind::Shared => false,
            BorrowKind::Mut => true,
        }
    }
}

#[derive(Eq, PartialEq, Hash)]
pub enum TyS<'lr> {
    /// A function type
    Fn {
        /// The input heap.
        in_heap: Heap<'lr>,
        /// Formal arguments of the function. These are always owned references so we
        /// represent them directly as locations in the input heap.
        params: Vec<OwnRef>,
        /// The output heap. This is right now only used to add a refinement for the returned
        /// reference but it should be extended to capture the state of the output heap.
        out_heap: Heap<'lr>,
        /// Location in the output heap of the returned owned reference.
        ret: OwnRef,
    },
    /// An owned reference
    OwnRef(Location),
    /// A mutable reference
    Ref(BorrowKind, Region, Location),
    /// A refinement type { bind: ty | pred }.
    Refine { ty: BasicType, pred: Pred<'lr> },
    /// A dependent tuple.
    Tuple(Vec<(Field, &'lr TyS<'lr>)>),
    /// Unitialized
    Uninit(u32),
    /// A refinment that need to be inferred
    RefineHole { ty: BasicType, n: u32 },
}

/// A type layout is used to describe the recursive structure of a type.
#[derive(Debug)]
pub enum TypeLayout {
    /// Tuples decompose memory recursively.
    Tuple(Vec<TypeLayout>),
    /// A block of memory that cannot be further decomposed recursively.
    Block(u32),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum BasicType {
    Bool,
    Int,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Field(pub Symbol);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum Var {
    Nu,
    Location(Location),
    Field(Field),
}

pub type Pred<'lr> = &'lr PredS<'lr>;

/// A refinement type predicate
#[derive(Eq, PartialEq, Hash)]
pub enum PredS<'lr> {
    Constant(Constant),
    Place { var: Var, projection: Vec<u32> },
    BinaryOp(BinOp, &'lr PredS<'lr>, &'lr PredS<'lr>),
    UnaryOp(UnOp, &'lr PredS<'lr>),
    Iff(&'lr PredS<'lr>, &'lr PredS<'lr>),
    Kvar(u32, Vec<Var>),
}

impl<'lr> TyS<'lr> {
    pub fn is_int(&self) -> bool {
        matches!(self, TyS::Refine { ty: BasicType::Int, .. })
    }

    pub fn is_copy(&self) -> bool {
        // TODO
        true
    }

    pub fn size(&self) -> u32 {
        match self {
            TyS::Fn { .. } | TyS::OwnRef(_) | TyS::Ref(..) => 1,
            TyS::Refine { .. } | TyS::RefineHole { .. } => 1,
            TyS::Tuple(fields) => fields.iter().map(|f| f.1.size()).sum(),
            TyS::Uninit(size) => *size,
        }
    }

    pub fn borrows<'a>(&'a self) -> Box<dyn Iterator<Item = (Path, BorrowKind, &Region)> + 'a> {
        self.borrows_(Path::empty())
    }

    fn borrows_<'a>(
        &'a self,
        path: Path,
    ) -> Box<dyn Iterator<Item = (Path, BorrowKind, &Region)> + 'a> {
        match self {
            TyS::Ref(kind, r, _) => Box::new(std::iter::once((path, *kind, r))),
            TyS::Tuple(fields) => Box::new(
                fields
                    .iter()
                    .enumerate()
                    .flat_map(move |(i, (_, t))| t.borrows_(path.append(i as u32))),
            ),
            TyS::Fn { .. }
            | TyS::OwnRef(_)
            | TyS::Refine { .. }
            | TyS::Uninit(_)
            | TyS::RefineHole { .. } => Box::new(std::iter::empty()),
        }
    }
}

impl Field {
    pub fn intern(string: &str) -> Self {
        Self(Symbol::intern(string))
    }

    pub fn nth(n: u32) -> Self {
        Self::intern(&format!("{}", n))
    }
}

impl From<Location> for Var {
    fn from(v: Location) -> Self {
        Var::Location(v)
    }
}

impl From<Field> for Var {
    fn from(f: Field) -> Self {
        Var::Field(f)
    }
}

impl Constant {
    pub fn ty(&self) -> BasicType {
        match self {
            Constant::Bool(_) => BasicType::Bool,
            Constant::Int(_) => BasicType::Int,
        }
    }
}

impl<'a> Into<TyS<'a>> for OwnRef {
    fn into(self) -> TyS<'a> {
        TyS::OwnRef(self.0)
    }
}

// -------------------------------------------------------------------------------------------------
// DEBUG
// -------------------------------------------------------------------------------------------------

impl Debug for Local {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &*self.0.as_str())
    }
}

impl Debug for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &*self.0.as_str())
    }
}

impl Debug for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Constant::Bool(b) => write!(f, "{}", b),
            Constant::Int(i) => write!(f, "{}", i),
        }
    }
}

impl Debug for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Lt => write!(f, "<"),
            BinOp::Le => write!(f, "<="),
            BinOp::Eq => write!(f, "="),
            BinOp::Ge => write!(f, ">="),
            BinOp::Gt => write!(f, ">"),
        }
    }
}

impl Debug for PredS<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PredS::Constant(c) => Debug::fmt(&c, f)?,
            PredS::Place { var, projection } => {
                write!(f, "{:?}", var)?;
                for p in projection {
                    write!(f, ".{}", p)?
                }
            }
            PredS::BinaryOp(op, lhs, rhs) => {
                write!(f, "({:?} {:?} {:?})", lhs, op, rhs)?;
            }
            PredS::UnaryOp(op, operand) => {
                write!(f, "{:?}({:?})", op, operand)?;
            }
            PredS::Iff(rhs, lhs) => {
                write!(f, "({:?} <=> {:?})", rhs, lhs)?;
            }
            PredS::Kvar(n, vars) => {
                write!(f, "$k{}{:?}", n, vars)?;
            }
        }
        Ok(())
    }
}

impl Debug for TyS<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TyS::Fn { .. } => write!(f, "function"),
            TyS::OwnRef(l) => write!(f, "OwnRef({:?})", l),
            TyS::Refine { ty, pred } => write!(f, "{{ {:?} | {:?} }}", ty, pred),
            TyS::Tuple(fields) => write!(f, "{:?}", fields),
            TyS::Uninit(size) => write!(f, "Uninit({})", size),
            TyS::RefineHole { ty, .. } => write!(f, "{{ {:?} | _ }}", ty),
            TyS::Ref(BorrowKind::Mut, r, l) => write!(f, "&mut({:?}, {:?})", r, l),
            TyS::Ref(BorrowKind::Shared, r, l) => write!(f, "&({:?}, {:?})", r, l),
        }
    }
}

impl Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl Debug for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Var::Nu => write!(f, "_v"),
            Var::Location(s) => write!(f, "l${}", *&s.0),
            Var::Field(s) => write!(f, "f${}", *&s.0),
        }
    }
}

impl Debug for UnOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnOp::Not => write!(f, "¬"),
        }
    }
}

impl Debug for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.local)?;
        for p in &self.projection {
            match p {
                Projection::Field(n) => write!(f, ".{:?}", n)?,
                Projection::Deref => write!(f, ".*")?,
            }
        }
        Ok(())
    }
}

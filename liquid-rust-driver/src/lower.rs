//! Lowering refinement annotations into the core IR.

use liquid_rust_core::{
    ast::{pred::{Place, Var}, Heap, FnTy, Pred, Refine, Ty},
    ty::{Location, UnOp, BinOp},
    names::Field,
};
use liquid_rust_parser::ast;
use quickscope::ScopeMap;

pub struct LowerCtx<'src> {
    vars: ScopeMap<Var<ast::Ident<'src>>, usize>,
    locs: usize,
}

impl<'src> LowerCtx<'src> {
    pub fn new() -> Self {
        LowerCtx {
            vars: ScopeMap::new(),
            locs: 0,
        }
    }

    pub fn fresh(&mut self) -> usize {
        self.locs += 1;
        self.locs - 1
    }

    pub fn define(&mut self, i: Var<ast::Ident<'src>>) -> usize {
        self.locs += 1;
        self.vars.define(i, self.locs);
        self.locs - 1
    }

    pub fn try_get(&mut self, i: Var<ast::Ident<'src>>) -> usize {
        if let Some(v) = self.vars.get(&i) {
            *v
        } else {
            self.define(i)
        }
    }
}

pub trait Lower<'src> {
    type Output;

    fn lower(self, lcx: &mut LowerCtx<'src>) -> Self::Output;
}

impl Lower<'_> for ast::UnOp {
    type Output = UnOp;

    fn lower(self, _lcx: &mut LowerCtx<'_>) -> Self::Output {
        match self.kind {
            ast::UnOpKind::Not => UnOp::Not,
            ast::UnOpKind::Neg => unimplemented!(),
        }
    }
}

impl Lower<'_> for ast::BinOp {
    type Output = BinOp;

    fn lower(self, _lcx: &mut LowerCtx<'_>) -> Self::Output {
        // TODO: Support iff (<=>)?
        match self.kind {
            ast::BinOpKind::Add => BinOp::Add,
            ast::BinOpKind::Sub => BinOp::Sub,
            ast::BinOpKind::Lt => BinOp::Lt,
            ast::BinOpKind::Le => BinOp::Le,
            ast::BinOpKind::Eq => BinOp::Eq,
            ast::BinOpKind::Ge => BinOp::Ge,
            ast::BinOpKind::Gt => BinOp::Gt,
            _ => unimplemented!(),
        }
    }
}

impl<'src> Lower<'src> for ast::Predicate<'src> {
    type Output = Pred;

    fn lower(self, lcx: &mut LowerCtx<'src>) -> Self::Output {
        match self.kind {
            ast::PredicateKind::Lit(c) => Pred::Constant(c),
            ast::PredicateKind::Place(p) => {
                let base = match p.place.base {
                    Var::Nu => Var::Nu,
                    l@Var::Location(_) => Var::Location(Location(lcx.try_get(l))),
                    f@Var::Field(_) => Var::Field(Field(lcx.try_get(f))),
                };
                let projs = p.place.projs;
                Pred::Place(Place { base, projs })
            }
            ast::PredicateKind::UnaryOp(uo, bp) => Pred::UnaryOp(uo.lower(lcx), Box::new((*bp).lower(lcx))),
            ast::PredicateKind::BinaryOp(bop, ba, bb) => Pred::BinaryOp(bop.lower(lcx), Box::new(ba.lower(lcx)), Box::new(bb.lower(lcx))),
        }
    }
}

impl<'src> Lower<'src> for ast::Ty<'src> {
    type Output = Ty;

    fn lower(self, lcx: &mut LowerCtx<'src>) -> Self::Output {
        match self.kind {
            ast::TyKind::Base(b) => Ty::Refine(
                b,
                Refine::Pred(Pred::Constant(ast::Constant::Bool(true))),
            ),
            // TODO: do something with ident
            // We assume it's same as arg
            ast::TyKind::Refined(_i, b, p) => {
                let lp = p.lower(lcx);
                Ty::Refine(b, Refine::Pred(lp))
            }
            ast::TyKind::Tuple(fs) => {
                lcx.vars.push_layer();
                let mut res = Vec::new();
                for (f, t) in fs {
                    let nf = Field(lcx.define(Var::Field(f.clone())));
                    res.push((nf, t.lower(lcx)));
                }
                lcx.vars.pop_layer();
                Ty::Tuple(res)
            }
        }
    }
}

impl<'src> Lower<'src> for ast::FnTy<'src> {
    type Output = FnTy;

    fn lower(self, lcx: &mut LowerCtx<'src>) -> Self::Output {
        let args = self.kind.args;
        let out = self.kind.output;

        let mut inputs = Vec::new();
        let mut in_heap = Vec::new();
        let mut out_heap = Vec::new();

        // We then iterate through each of the args and lower each of them.
        for (ident, ty) in args {
            // Generate a fresh location which will be used in the input
            // heap
            let loc = Location(lcx.try_get(Var::Location(Location(ident.clone()))));

            // We lower the target type
            let lty = ty.lower(lcx);

            // We then insert the arg into the inputs and the heap.
            inputs.push(loc.clone());
            in_heap.push((loc.clone(), lty.clone()));
            out_heap.push((loc, lty));
        }

        // Afterwards, we lower the output.
        // TODO: This is an ugly hack, and at some point
        // we should change this whole file to generate FnTy<usize>,
        // handling the location generation ourselves with our own
        // scopemap.
        let output = Location(lcx.fresh());
        let oty = (*out).lower(lcx);
        out_heap.push((output.clone(), oty));

        FnTy {
            in_heap: Heap(in_heap),
            inputs,
            out_heap: Heap(out_heap),
            output,
        }
    }
}

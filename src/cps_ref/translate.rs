//! Handles the translation from Rust MIR to the CPS IR.

use super::ast::*;
use super::context as cps_ctx;
use crate::{context::LiquidRustCtxt, refinements::dom_tree::DominatorTree};
use rustc_data_structures::graph::WithStartNode;
use rustc_mir::transform::MirSource;
use rustc_middle::{
    mir::{self, terminator::TerminatorKind, Body, StatementKind},
    ty,
};
use rustc_span::Symbol;
use std::{collections::HashMap, convert::TryInto, mem::size_of};

// First, we have to convert the MIR code to an SSA form.
// Once we do this, we can convert the SSA form into
// CPS form.

/// Translates an mir::Place to a CPS IR Place.
fn translate_place(from: &mir::Place) -> Place {
    let local = Local(Symbol::intern(format!("_{}", from.local.as_u32()).as_str()));
    let mut projection = vec![];

    for proj in from.projection {
        match proj {
            mir::ProjectionElem::Field(f, _ty) => projection.push(f.as_u32()),
            mir::ProjectionElem::Deref => unimplemented!(),
            _ => todo!(),
        };
    }

    Place { local, projection }
}

fn translate_op(from: &mir::Operand) -> Operand {
    match from {
        mir::Operand::Copy(p) => Operand::Deref(translate_place(p)),
        mir::Operand::Move(p) => Operand::Deref(translate_place(p)),
        mir::Operand::Constant(_bc) => unimplemented!(),
    }
}

fn translate_rvalue<'tcx>(from: &mir::Rvalue<'tcx>) -> Rvalue {
    match from {
        mir::Rvalue::Use(op) => Rvalue::Use(translate_op(op)),
        mir::Rvalue::BinaryOp(op, a, b) => {
            Rvalue::BinaryOp((*op).into(), translate_op(a), translate_op(b))
        }
        mir::Rvalue::CheckedBinaryOp(op, a, b) => {
            Rvalue::CheckedBinaryOp((*op).into(), translate_op(a), translate_op(b))
        }
        _ => todo!(),
    }
}

impl From<mir::BinOp> for BinOp {
    fn from(op: mir::BinOp) -> BinOp {
        match op {
            mir::BinOp::Add => BinOp::Add,
            mir::BinOp::Sub => BinOp::Sub,
            mir::BinOp::Lt => BinOp::Lt,
            mir::BinOp::Le => BinOp::Le,
            mir::BinOp::Eq => BinOp::Eq,
            mir::BinOp::Ge => BinOp::Ge,
            mir::BinOp::Gt => BinOp::Gt,
            _ => todo!(),
        }
    }
}

fn get_basic_type<'tcx>(t: ty::Ty<'tcx>) -> BasicType {
    match &t.kind {
        ty::TyKind::Bool => BasicType::Bool,
        ty::TyKind::Int(_) | ty::TyKind::Uint(_) => BasicType::Int,
        _ => todo!(),
    }
}

/// Creates a TypeLayout based on a Rust TyKind.
fn get_layout<'tcx>(t: ty::Ty<'tcx>) -> TypeLayout {
    // Get the Rust type for ints, bools, tuples (of ints, bools, tuples)
    // Do case analysis, generate TypeLayout based on that.
    // Give up if not supported type
    match &t.kind {
        ty::TyKind::Bool => TypeLayout::Block(size_of::<bool>().try_into().unwrap()),
        ty::TyKind::Int(it) => TypeLayout::Block(it.bit_width().map(|x| x >> 3).unwrap_or_else(|| size_of::<isize>().try_into().unwrap()) as u32),
        ty::TyKind::Uint(it) => TypeLayout::Block(it.bit_width().map(|x| x >> 3 as u32).unwrap_or_else(|| size_of::<isize>().try_into().unwrap()) as u32),
        ty::TyKind::Tuple(_) => TypeLayout::Tuple(t.tuple_fields().map(|c| get_layout(c)).collect::<Vec<_>>()),
        _ => todo!(),
    }
}

// Transformer state struct should include a mapping from locals to refinements too

pub struct Transformer<'a, 'lr, 'tcx> {
    cx: &'a LiquidRustCtxt<'lr, 'tcx>,
    // TODO: What should the lifetime on this be?
    cps_cx: &'lr cps_ctx::LiquidRustCtxt<'lr>,
    tcx: ty::TyCtxt<'tcx>,
    symbols: HashMap<Symbol, usize>,
    holes: u32,
}

impl<'a, 'lr, 'tcx> Transformer<'a, 'lr, 'tcx> {
    pub fn new(cx: &'a LiquidRustCtxt<'lr, 'tcx>, cps_cx: &'lr cps_ctx::LiquidRustCtxt<'lr>) -> Self {
        Self {
            cx,
            cps_cx,
            tcx: cx.tcx(),
            symbols: HashMap::new(),
            holes: 0,
        }
    }

    /// Generates a fresh variable with a certain prefix.
    fn fresh(&mut self, prefix: Symbol) -> Symbol {
        // We look up our symbol in our map.
        // If it doesn't already exist, return it suffixed by 0.
        // Otherwise, return it with the correct prefix.
        // In both cases, we only return if the symbol with the suffix
        // also doesn't exist.

        let sym = if let Some(s) = self.symbols.get_mut(&prefix) {
            let sym = Symbol::intern(format!("{}{}", &prefix, *s).as_str());
            *s += 1;
            sym
        } else {
            let sym = Symbol::intern(format!("{}0", &prefix).as_str());
            self.init_sym(sym);
            sym
        };

        if self.symbols.get(&sym).is_none() {
            sym
        } else {
            self.fresh(sym)
        }
    }

    /// Records a symbol as being used
    fn init_sym(&mut self, sym: Symbol) {
        self.symbols.insert(sym, 1);
    }

    fn fresh_hole(&mut self) -> u32 {
        self.holes += 1;
        return self.holes;
    }

    fn mk_refine_hole(&mut self, bty: BasicType) -> Ty<'lr> {
        self.cps_cx.mk_refine_hole(bty, self.fresh_hole())
    }

    /// Based on the structure of the type, return either a RefineHole
    /// or a tuple of holy types.
    fn get_holy_type(&mut self, t: ty::Ty<'tcx>) -> Ty<'lr> {
        match &t.kind {
            ty::TyKind::Tuple(_) => self.cps_cx.mk_tuple(&t.tuple_fields().enumerate().map(|(i, f)| {
                (Field::nth(i.try_into().unwrap()), self.get_holy_type(f))
            }).collect::<Vec<_>>()),
            _ => self.mk_refine_hole(get_basic_type(t)),
        }
    }

    // TODO: In later compiler versions, the MirSource is contained as a field
    // source within the Body
    /// Translates an MIR function body to a CPS IR FnDef.
    pub fn translate_body(&mut self, source: MirSource<'tcx>, body: &Body<'tcx>) -> FnDef<'lr> {
        let retk = self.fresh(Symbol::intern("_rk"));

        // The let-bound local representing the return value of the function
        let retv = Symbol::intern("_0");

        // We first generate a jump instruction to jump to the continuation
        // corresponding to the first/root basic block, bb0.
        let mut nb = FnBody::Jump {
            target: Symbol::intern("bb0"),
            args: Vec::new(),
        };

        // We then iterate through each basic block in reverse breadth-first dominator
        // tree order
        let dom_tree = DominatorTree::build(&self.cx, body);
        let bbs = dom_tree
            .bfs(body.start_node())
            .map(|(_depth, _pred, bb)| bb)
            .collect::<Vec<_>>();

        for bb in bbs.iter().rev() {
            let bbd = &body[*bb];

            // For each basic block, we generate a statement for the terminator first,
            // then we go through the statements in reverse, building onto the
            // FnBody this way.
            let mut bbod = match &bbd.terminator().kind {
                TerminatorKind::Goto { target } => FnBody::Jump {
                    target: Symbol::intern(format!("bb{}", target.as_u32()).as_str()),
                    args: Vec::new(),
                },
                TerminatorKind::SwitchInt {
                    discr,
                    targets,
                    values,
                    ..
                } => {
                    // We have to test our operand against each provided target value.
                    // This will turn into nested conditionals: if {} else { if ... }

                    // We first start with the else branch, since that's at the leaf of our
                    // if-else-if-else chain, and build backwards from there.
                    let mut tgs = targets.iter().rev();

                    let otherwise = tgs.next().unwrap();
                    // TODO: pass in actual args
                    let mut ite = FnBody::Jump {
                        target: Symbol::intern(format!("bb{}", otherwise.as_u32()).as_str()),
                        args: vec![],
                    };

                    // Then for each value remaining, we create a new FnBody::Ite, jumping to
                    // the specified target.
                    for (val, target) in values.iter().zip(tgs) {
                        // We first have to translate our discriminator into an AST Operand.
                        let op = translate_op(discr);

                        // TODO: pass in actual args
                        let then = FnBody::Jump {
                            target: Symbol::intern(format!("bb{}", target.as_u32()).as_str()),
                            args: vec![],
                        };

                        // We can only have places for guards, so we have
                        // to create a place first.
                        let sym = Local(self.fresh(Symbol::intern(format!("_g").as_str())));
                        // Bools are guaranteed to be one byte, so assuming a one byte
                        // TypeLayout should be ok!
                        let bind = Statement::Let(sym, TypeLayout::Block(size_of::<bool>().try_into().unwrap()));

                        let pl = Place {
                            local: sym,
                            projection: vec![],
                        };
                        let asgn = Statement::Assign(
                            pl.clone(),
                            Rvalue::BinaryOp(BinOp::Eq, op, Operand::Constant(Constant::Int(*val))),
                        );

                        ite = FnBody::Seq(
                            bind,
                            Box::new(FnBody::Seq(
                                asgn,
                                Box::new(FnBody::Ite {
                                    discr: pl,
                                    then: Box::new(then),
                                    else_: Box::new(ite),
                                }),
                            )),
                        );
                    }

                    // Finally, return the ite.
                    ite
                }
                // For returning, we call the return continuation on _0, the let-bound local representing
                // the return value
                TerminatorKind::Return => FnBody::Jump {
                    target: retk,
                    args: vec![Local(retv)],
                },
                TerminatorKind::Call {
                    func,
                    args,
                    destination,
                    ..
                } => {
                    // TODO: For now, we assume that all functions are constants (i.e. they're defined
                    // separately outside of this function. This isn't always true, however.

                    // We first get the destination basic block out of the destination; we'll
                    // do the assignment to the place after we have our FnBody::Call
                    // If destination is None, that means that this function doesn't converge;
                    // it diverges and never returns (i.e. returns ! and infinitely loops or smth)
                    // TODO: Perhaps handle the diverging case somehow?
                    let ret = match destination {
                        Some((_, bb)) => Symbol::intern(format!("_{}", bb.as_u32()).as_str()),
                        None => todo!(),
                    };

                    // For our args, our args will be a list of new temp locals that we create.
                    // We'll actually create these locals after we have our FnBody::Call, so that
                    // we can reference it.
                    let start_ix = *self
                        .symbols
                        .get(&Symbol::intern(format!("_farg").as_str()))
                        .unwrap_or(&0);
                    let new_args = (start_ix..start_ix + args.len())
                        .map(|i| Local(Symbol::intern(format!("_farg{}", i).as_str())))
                        .collect::<Vec<_>>();

                    let mut fb = match func {
                        mir::Operand::Constant(bc) => {
                            let c = &*bc;
                            let kind = &c.literal.ty.kind;

                            match kind {
                                ty::TyKind::FnDef(def_id, _) => {
                                    // We get the stringified name of this def,
                                    // then use it as the name of the function
                                    // we're calling.

                                    let fname = self.tcx.def_path_str(*def_id);
                                    let func = Place {
                                        local: Local(Symbol::intern(&fname)),
                                        projection: vec![],
                                    };

                                    // Finally, return our FnBody::Call!
                                    FnBody::Call {
                                        func,
                                        args: new_args,
                                        ret,
                                    }
                                }
                                _ => unreachable!(),
                            }
                        }
                        _ => todo!(),
                    };

                    // We now have to actually create and assign locals for our operands.
                    for arg in args {
                        // We let-define a new variable for our function arg, then
                        // assign it to the value of the arg.

                        let sym = Local(self.fresh(Symbol::intern(format!("_farg").as_str())));
                        let tys = arg.ty(body, self.tcx);
                        let bind = Statement::Let(sym, get_layout(&tys));

                        let pl = Place {
                            local: sym,
                            projection: vec![],
                        };
                        let assign = Statement::Assign(pl, Rvalue::Use(translate_op(arg)));
                        fb = FnBody::Seq(bind, Box::new(FnBody::Seq(assign, Box::new(fb))));
                    }

                    fb
                }
                TerminatorKind::Abort => FnBody::Abort,
                _ => todo!(),
            };

            for stmt in bbd.statements.iter().rev() {
                match &stmt.kind {
                    StatementKind::Assign(pr) => {
                        let place = translate_place(&pr.0);
                        let rval = translate_rvalue(&pr.1);

                        let stmt = Statement::Assign(place, rval);
                        bbod = FnBody::Seq(stmt, Box::new(bbod));
                    }

                    _ => { /* TODO? */ }
                };
            }

            // We update our body here
            // TODO: Fill this out with proper things

            // For now, for our continuations, we use all of the locals
            // as our env arguments, keeping the parameters empty.
            // These env arguments point to heap locations, where the BasicType
            // corresponds to the type of the local, and the refinement is a
            // hole (we use RefineHole)

            let mut env = vec![];
            let mut heap = vec![];

            for (lix, decl) in body.local_decls.iter_enumerated() {
                let arg = Local(Symbol::intern(format!("_{}", lix.index()).as_str()));
                let loc = Location(Symbol::intern(format!("loc_{}", lix.index()).as_str()));
                let ty = self.get_holy_type(decl.ty);

                env.push((arg, OwnRef(loc)));
                heap.push((loc, ty));
            }

            let lc = ContDef {
                name: Symbol::intern(format!("bb{}", bb.as_u32()).as_str()),
                heap,
                env,
                params: vec![],
                body: Box::new(bbod),
            };
            
            nb = FnBody::LetCont {
                def: lc,
                rest: Box::new(nb),
            };
        }

        // We finish by taking care of the let bindings - let binding all of the
        // locals in our MIR function body.
        // We do this because a FnBody::Sequence takes a statement and the rest
        // of the function body; we do this at the end so that we have a "rest of`
        // the function body"
        for (ix, decl) in body.local_decls.iter_enumerated().rev() {
            if (1..body.arg_count + 1).contains(&ix.index()) {
                // Skip over argument locals, they're printed in the signature.
                continue;
            }

            let sym = Local(Symbol::intern(format!("_{}", ix.as_u32()).as_str()));
            let s = Statement::Let(sym, get_layout(decl.ty));
            nb = FnBody::Seq(s, Box::new(nb));
        }

        // For our function definition, we need to record what arguments our
        // function takes
        // As with our continuation, our function args go in args; all of
        // the args are owned references to vars in the heap. Each of these
        // vars has a corresponding BasicType, refined with a RefineHole

        let mut args = vec![];
        let mut heap = vec![];

        for lix in body.args_iter() {
            let decl = &body.local_decls[lix];

            let arg = Local(Symbol::intern(format!("_{}", lix.index()).as_str()));
            let loc = Location(Symbol::intern(format!("loc_{}", lix.index()).as_str()));
            let ty = self.get_holy_type(decl.ty);

            args.push((arg, OwnRef(loc)));
            heap.push((loc, ty));
        }

        // Our return type is local _0; we want to get a holy type here as
        // our return type
        let out_loc = Location(Symbol::intern(format!("loc_0").as_str()));
        let out_tys = self.get_holy_type(body.local_decls[mir::Local::from_u32(0)].ty);
        heap.push((out_loc, out_tys));
        let out_ty = OwnRef(out_loc);

        // Return our translated body
        // TODO: Different out_heap than input heap?
        FnDef {
            name: Symbol::intern(self.tcx.def_path_str(source.def_id()).as_str()),
            heap: heap.clone(),
            args,
            ret: retk,
            out_heap: heap,
            out_ty,
            body: Box::new(nb),
        }
    }
}

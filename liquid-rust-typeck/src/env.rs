use crate::subtyping::subtyping;
use ast::Proj;
use liquid_rust_core::{
    ast,
    lower::TypeLowerer,
    names::{ContId, Local, Location},
    ty::{self, pred::Place, Heap, LocalsMap, Ty, TyCtxt},
};
use quickscope::ScopeMap;
use std::fmt;
use ty::{BorrowKind, TyKind};

use crate::constraint::Constraint;

pub struct Env<'a> {
    tcx: &'a TyCtxt,
    locals: Vec<LocalsMap>,
    heap: Heap,
}

impl<'a> Env<'a> {
    pub fn new(tcx: &'a TyCtxt) -> Self {
        Env {
            tcx,
            locals: vec![LocalsMap::empty()],
            heap: Heap::new(),
        }
    }
}

impl Env<'_> {
    pub fn alloc(&mut self, x: Local, ty: Ty) {
        let l = self.fresh_location();
        self.insert_local(x, l);
        self.heap.insert(l, ty);
    }

    pub fn update(&mut self, place: &ast::Place, ty: Ty) {
        let l = self.lookup_local(&place.base);
        let root = self.tcx.selfify(self.lookup_location(l), Place::from(*l));

        let fresh_l = self.fresh_location();
        let ty = self.update_ty(&root, &place.projs, ty);
        self.insert_local(place.base, fresh_l);
        self.heap.insert(fresh_l, ty);
    }

    pub fn borrow(&mut self, place: &ast::Place) -> Location {
        let ty = self.lookup(place).clone();
        let l = self.fresh_location();
        self.heap.insert(l, ty);
        l
    }

    pub fn drop(&mut self, x: &Local) -> Constraint {
        let l = self.lookup_local(x);
        let ty = self.lookup_location(l).clone();
        let constraint = self.drop_ty(&ty);

        let fresh_l = self.fresh_location();
        self.insert_local(*x, fresh_l);
        self.heap.insert(fresh_l, self.tcx.mk_uninit(ty.size()));
        constraint
    }

    pub fn lookup(&self, place: &ast::Place) -> &Ty {
        let mut ty = self.lookup_location(self.lookup_local(&place.base));
        for p in &place.projs {
            match (ty.kind(), p) {
                (TyKind::Tuple(tuple), &Proj::Field(n)) => {
                    ty = tuple.ty_at(n);
                }
                (TyKind::Ref(.., l), Proj::Deref) => {
                    ty = self.lookup_location(l);
                }
                _ => bug!("{:?} {:?} {:?}", ty, place, p),
            }
        }
        ty
    }

    pub fn resolve_place(&self, place: &ast::Place) -> Place {
        let mut base = *self.lookup_local(&place.base);
        let mut ty = self.lookup_location(&base);

        let mut projs = Vec::new();
        for proj in &place.projs {
            match (ty.kind(), proj) {
                (TyKind::Tuple(tup), &Proj::Field(n)) => {
                    ty = tup.ty_at(n);
                    projs.push(n);
                }
                (TyKind::Ref(.., l), Proj::Deref) => {
                    projs.clear();
                    base = *l;
                    ty = self.lookup_location(l);
                }
                _ => bug!(),
            }
        }

        Place {
            base: ty::Var::from(base),
            projs,
        }
    }

    pub fn resolve_operand(&self, op: &ast::Operand) -> ty::Pred {
        match op {
            ast::Operand::Use(place) => self.tcx.mk_pred_place(self.resolve_place(place)),
            ast::Operand::Constant(c) => {
                let c = match *c {
                    ast::Constant::Bool(b) => ty::pred::Constant::Bool(b),
                    ast::Constant::Int(n) => ty::pred::Constant::Int(n),
                    ast::Constant::Unit => ty::pred::Constant::Unit,
                };
                self.tcx.mk_constant(c)
            }
        }
    }

    pub fn extend_heap(&mut self, heap: &Heap) {
        for (l, ty) in heap {
            self.heap.insert(*l, ty.clone());
        }
    }

    pub fn insert_locals(&mut self, locals: LocalsMap) {
        self.locals.last_mut().unwrap().extend(locals);
    }

    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    pub fn locals(&self) -> &LocalsMap {
        self.locals.last().unwrap()
    }

    pub fn vars_in_scope(&self) -> Vec<ty::Var> {
        self.heap.keys().map(|&l| ty::Var::Location(l)).collect()
    }

    pub fn snapshot(&mut self) -> Snapshot {
        let heap_len = self.heap.len();
        let locals_depth = self.locals.len();

        self.locals.push(self.locals.last().unwrap().clone());
        Snapshot {
            heap_len,
            locals_depth,
        }
    }

    pub fn snapshot_without_locals(&mut self) -> Snapshot {
        let heap_len = self.heap.len();
        let locals_depth = self.locals.len();

        self.locals.push(LocalsMap::empty());
        Snapshot {
            heap_len,
            locals_depth,
        }
    }

    pub fn rollback_to(&mut self, snapshot: Snapshot) {
        self.heap.truncate(snapshot.heap_len);
        self.locals.truncate(snapshot.locals_depth);
    }

    pub fn capture_bindings<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> T,
    ) -> (T, Vec<(Location, Ty)>) {
        let n = self.heap.len();
        let r = f(self);
        let bindings = self
            .heap
            .iter()
            .skip(n)
            .map(|(l, ty)| (*l, ty.clone()))
            .collect();
        (r, bindings)
    }

    // Private

    fn insert_local(&mut self, x: Local, l: Location) {
        self.locals.last_mut().unwrap().insert(x, l);
    }

    fn lookup_local(&self, x: &Local) -> &Location {
        self.locals
            .last()
            .unwrap()
            .get(x)
            .expect(&format!("Env: local not found {:?}", x))
    }

    fn lookup_location(&self, l: &Location) -> &Ty {
        self.heap
            .get(l)
            .expect(&format!("Env: location not found {:?}", l))
    }

    fn fresh_location(&self) -> Location {
        self.tcx.fresh_location()
    }

    fn update_ty(&mut self, root: &Ty, projs: &[Proj], ty: Ty) -> Ty {
        match (root.kind(), projs) {
            (_, []) => ty,
            (ty::TyKind::Tuple(tup), [Proj::Field(n), ..]) => {
                let ty = self.update_ty(tup.ty_at(*n), &projs[1..], ty);
                self.tcx.mk_tuple(tup.map_ty_at(*n, |_| ty))
            }
            (ty::TyKind::Ref(bk, r, l), [Proj::Deref, ..]) => {
                let root = self.tcx.selfify(self.lookup_location(l), Place::from(*l));

                let fresh_l = self.fresh_location();
                let ty = self.update_ty(&root, &projs[1..], ty);
                self.heap.insert(fresh_l, ty);
                self.tcx.mk_ref(*bk, r.clone(), fresh_l)
            }
            (ty::TyKind::OwnRef(l), [Proj::Deref, ..]) => {
                let root = self.tcx.selfify(self.lookup_location(l), Place::from(*l));

                let fresh_l = self.fresh_location();
                let ty = self.update_ty(&root, &projs[1..], ty);
                self.heap.insert(fresh_l, ty);
                self.tcx.mk_own_ref(fresh_l)
            }
            _ => bug!(),
        }
    }

    fn drop_ty(&mut self, ty: &Ty) -> Constraint {
        let vars_in_scope = self.vars_in_scope();
        let mut constraints = vec![];
        ty.walk(&mut |ty| match ty.kind() {
            TyKind::Ref(BorrowKind::Mut, r, l) => {
                let ty = self.lookup_location(l).clone();
                let places = match r {
                    ty::Region::Concrete(places) => places.as_slice(),
                    ty::Region::Infer(_) => &[],
                };
                match places {
                    [] => {}
                    [place] => {
                        self.update(place, ty);
                    }
                    _ => {
                        let ty_join = self.tcx.replace_refines_with_kvars(&ty, &vars_in_scope);

                        let heap = self.heap();
                        constraints.push(subtyping(self.tcx, heap, &ty, heap, &ty_join));
                        for place in places {
                            let ty = self.lookup(place);
                            constraints.push(subtyping(self.tcx, heap, &ty, heap, &ty_join));
                        }

                        for place in places {
                            self.update(place, ty_join.clone());
                        }
                    }
                }
            }
            _ => {}
        });
        Constraint::Conj(constraints)
    }
}

pub struct Snapshot {
    heap_len: usize,
    locals_depth: usize,
}

impl fmt::Debug for Env<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n", self.heap)?;
        let s = self
            .locals
            .last()
            .unwrap()
            .iter()
            .map(|(x, l)| format!("$x{} => $l{}", x.0, l.0))
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "[{}]", s)
    }
}

pub struct ContEnv<'a> {
    map: ScopeMap<ty::ContId, ty::ContTy>,
    tcx: &'a TyCtxt,
}

impl<'a> ContEnv<'a> {
    pub fn new(tcx: &'a TyCtxt) -> Self {
        Self {
            map: ScopeMap::new(),
            tcx,
        }
    }

    pub fn define_cont(
        &mut self,
        cont_id: ContId,
        cont_ty: &ast::ContTy,
        vars_in_scope: Vec<ty::Var>,
    ) {
        self.map.define(
            cont_id,
            TypeLowerer::new(self.tcx, vars_in_scope).lower_cont_ty(cont_ty),
        )
    }

    pub fn define_ret_cont(
        &mut self,
        cont_id: ContId,
        fn_ty: &ast::FnTy,
        vars_in_scope: Vec<ty::Var>,
    ) {
        self.map.define(
            cont_id,
            ty::ContTy::new(
                TypeLowerer::new(self.tcx, vars_in_scope).lower_heap(&fn_ty.out_heap),
                LocalsMap::empty(),
                vec![fn_ty.output],
            ),
        )
    }

    pub fn get_ty(&self, cont_id: &ContId) -> Option<&ty::ContTy> {
        self.map.get(cont_id)
    }
}

impl<'a> std::ops::Index<&'a ContId> for ContEnv<'_> {
    type Output = ty::ContTy;

    fn index(&self, index: &'a ContId) -> &Self::Output {
        &self.get_ty(index).expect("ContEnv: continuation not found")
    }
}

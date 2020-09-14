pub mod ast;
pub mod parser;
pub mod typeck;

#[cfg(test)]
mod tests {
    use rustc_ast::attr::with_default_session_globals;

    #[test]
    fn abs() {
        with_default_session_globals(|| {
            let expr = super::parser::FnParser::new().parse(
                r####"
fn abs(n0: {v: int | true}; n: n0) ret k(r: {v:int | v >= 0}; r) =
  let b = new(1);
  b := *n < 0;
  if *n then
    n := 0 - *n;
    jump k(n)
  else
    jump k(n)
"####,
            );
            assert!(expr.is_ok());
        });
    }

    #[test]
    fn sum() {
        with_default_session_globals(|| {
            let expr = super::parser::FnParser::new().parse(
                r####"
fn sum(n0: {v: int | v >= 0}; n: n0) ret k(r: {v:int | v >= 0}; r) =
  letcont loop(i1: {v: int | v >= 0}, r1: {v: int | v >= i1}; i: own(i1), r: own(r);) =
    let t0 = new(1);
    t0 := *i <= *n;
    if *t0 then
      let t1 = new(1);
      t1 := *r + *i;
      jump loop()
    else
      jump k(r)
  in
  let i = new(1);
  let r = new(1);
  i := 0;
  r := 0;
  jump loop()
"####,
            );
            assert!(expr.is_ok());
        });
    }

    #[test]
    fn count_zeros() {
        with_default_session_globals(|| {
            let expr = super::parser::FnParser::new().parse(
                r####"
fn count_zeros(n0: {v: int | v >= 0}; n: n0) ret k(r: {v: int | v >= 0}; r)=
  letcont b0(i1: {v: int | v >= 0}, c1: {v: int | v >= 0}; i: own(i1), c: own(c1); ) =
    let t0 = new(1);
    t0 := *i < *n;
    if *t0 then
      letcont b1( i2: {v: int | v >= 0}, c2: {v: int | v >= 0}, x0: {v: int | true}
                ; i: own(i2), c: own(c2)
                ; x: x0
                ) =
        let t1 = new(1);
        t1 := *x == 0;
        if *t1 then
          c := *c + 1;
          jump b0()
        else
          jump b0()
      in
      call f(i) ret b1
    else
      jump k(c)
  in
  let i = new(1);
  let c = new(1);
  i := 0;
  c := 0;
  jump b0()
"####,
            );
            assert!(expr.is_ok());
        });
    }

    #[test]
    fn alloc_pair() {
        with_default_session_globals(|| {
            let expr = super::parser::FnParser::new().parse(
                r####"
fn alloc_pair(;) ret k(r: {v: int | true}; r)=
  let p = new((1, 1));
  t.0 := 1;
  t.1 := 2;
  let r = new(1);
  r := *p.0;
  jump k(r)
"####,
            );
            assert!(expr.is_ok());
        });
    }

    #[test]
    fn length() {
        with_default_session_globals(|| {
            let expr = super::parser::FnParser::new().parse(
                r####"
fn length(p0: (x: {v: int | true}, y: {v: int | v >= x}); p: p0) ret k(r: {v: int | v >= 0}; r)=
  let t = new(1);
  t := *p.1 - *p.0;
  jump k(t)
"####,
            );
            assert!(expr.is_ok());
        });
    }
}

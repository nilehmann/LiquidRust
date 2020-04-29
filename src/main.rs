#![feature(rustc_private)]
#![feature(box_syntax)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;

use rustc_driver::{Callbacks, Compilation};
use rustc_interface::{interface::Compiler, Config, Queries};
use rustc_lint::{LateContext, LateLintPass, LintPass};

struct LiquidRustLintPass;

impl LiquidRustLintPass {
    fn new() -> LiquidRustLintPass {
        LiquidRustLintPass
    }
}

impl LintPass for LiquidRustLintPass {
    fn name(&self) -> &'static str {
        return stringify!(LiquidRust);
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for LiquidRustLintPass {
    fn check_crate(&mut self, cx: &LateContext<'a, 'tcx>, krate: &'tcx rustc_hir::Crate<'tcx>) {
        let _ = liquid_rust::run(cx, krate);
    }
}

struct LiquidRustDriver;

impl Callbacks for LiquidRustDriver {
    fn config(&mut self, config: &mut Config) {
        config.register_lints = Some(box move |_sess, lint_store| {
            lint_store.register_late_pass(move || box LiquidRustLintPass::new());
        });
    }

    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &Compiler,
        _queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        Compilation::Stop
    }
}

fn sys_root() -> Vec<String> {
    let home = option_env!("RUSTUP_HOME");
    let toolchain = option_env!("RUSTUP_TOOLCHAIN");
    let sysroot = format!("{}/toolchains/{}", home.unwrap(), toolchain.unwrap());
    vec!["--sysroot".into(), sysroot]
}

fn allow_unused_doc_comments() -> Vec<String> {
    vec!["-A".into(), "unused_doc_comments".into()]
}

fn main() {
    let _ = rustc_driver::catch_fatal_errors(|| {
        // Grab the command line arguments.
        let args: Vec<_> = std::env::args_os().flat_map(|s| s.into_string()).collect();
        let args2 = args
            .iter()
            .map(|s| (*s).to_string())
            // Note: if running without rustup, comment the next line out
            // and manually pass in the rust compiler dir via --sysroot
            .chain(sys_root().into_iter())
            .chain(allow_unused_doc_comments().into_iter())
            .collect::<Vec<_>>();

        rustc_driver::run_compiler(&args2, &mut LiquidRustDriver, None, None)
    })
    .map_err(|e| println!("{:?}", e));
}

//! Actually building Idris packages.

pub mod context;
pub mod invoke;
pub mod job;

use self::{context::BuildContext, invoke::CompileInvocation};
use retrieve::cache::{Binary, OutputLayout, Source};
use std::fs;
use util::errors::Res;

/// The general "mode" of what to do
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
pub enum CompileMode {
    /// Typecheck a target without codegen
    Lib,
    /// Compile and codegen executable(s)
    ///
    /// This subsumes the "Bench" and "Test" modes since those are just compiling and running
    /// executables anyway
    ///
    /// The argument is the path(s) to the binar(y/ies).
    Bin,
    /// Create documentation
    Doc,
}

pub fn compile_lib(
    source: &Source,
    deps: &[&Binary],
    layout: &OutputLayout,
    bcx: &BuildContext,
) -> Res<()> {
    let lib_target = source.meta().targets.lib.clone().ok_or_else(|| {
        format_err!(
            "package {} does not contain lib target",
            source.meta().package.name
        )
    })?;

    let src_path = source.path().join(&lib_target.path);
    let targets = lib_target
        .mods
        .iter()
        .map(|mod_name| {
            src_path
                .join(mod_name.replace(".", "/"))
                .with_extension("idr")
        })
        .collect::<Vec<_>>();

    let invocation = CompileInvocation {
        src: &source.path().join(&lib_target.path),
        deps,
        targets: &targets,
        layout: &layout,
    };

    invocation.exec(bcx)?;

    for from in targets {
        let to = layout.build.join(from.strip_prefix(&src_path).unwrap());

        fs::create_dir_all(to.parent().unwrap())?;
        fs::rename(from, to)?;
    }

    Ok(())
}

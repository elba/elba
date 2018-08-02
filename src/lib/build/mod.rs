//! Actually building Idris packages.

pub mod context;
pub mod invoke;
pub mod job;

use self::{context::BuildContext, invoke::CompileInvocation};
use retrieve::cache::{Binary, OutputLayout, Source};
use std::fs;
use util::{clear_dir, errors::Res};

/// A type of Target that should be built
#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Debug, Eq, Hash)]
pub enum Target {
    /// Typecheck a library without codegen
    Lib,
    /// Compile a standalone executable which doesn't require the package's lib to be
    /// built
    ///
    /// The usize field is the index of the BinTarget in the manifest's list of BinTargets which
    /// should be built
    Bin(usize),
    // Both Test and Bench are like Bin, except that they require the lib to be built already.
    Test(usize),
    Bench(usize),
    // I would assume creating documentation requires the lib to be built too
    /// Create documentation
    Doc,
}

#[derive(Clone, PartialEq, Debug, Eq, Hash)]
pub struct Targets(pub Vec<Target>);

impl Targets {
    pub fn new(mut ts: Vec<Target>) -> Self {
        ts.sort();
        
        let mut res = vec![];
        
        let mut seen_lib = false;
        
        for i in ts {
            match i {
                Target::Lib => {
                    if !seen_lib {
                        seen_lib = true;
                        res.push(i);
                    }
                }
                Target::Bin(_) => {}
                Target::Test(_) => {
                    if !seen_lib {
                        seen_lib = true;
                        res.insert(0, Target::Lib);
                        res.push(i);
                    }
                }
                Target::Bench(_) => {
                    if !seen_lib {
                        seen_lib = true;
                        res.insert(0, Target::Lib);
                        res.push(i);
                    }
                }
                Target::Doc => {
                    if !seen_lib {
                        seen_lib = true;
                        res.insert(0, Target::Lib);
                        res.push(i);
                    }
                }
            }
        }
        
        Targets(res)
    }
}

pub fn compile_lib(
    source: &Source,
    deps: &[&Binary],
    layout: &OutputLayout,
    bcx: &BuildContext,
) -> Res<()> {
    let lib_target = source.meta().targets.lib.clone().ok_or_else(|| {
        format_err!(
            "package {} doesn't contain a lib target",
            source.meta().package.name
        )
    })?;
    
    clear_dir(&layout.lib)?;

    // We know that lib_target.path will be relative to the package root
    let src_path = source.path().join(&lib_target.path.0);
    let targets = lib_target
        .mods
        .iter()
        .map(|mod_name| {
            lib_target.path.0
                .join(mod_name.replace(".", "/"))
                .with_extension("idr")
        })
        .collect::<Vec<_>>();

    let invocation = CompileInvocation {
        src: &src_path,
        deps,
        targets: &targets,
        layout: &layout,
    };

    invocation.exec(bcx)?;

    for target in targets {
        let target_bin = target.with_extension("ibc");
        let from = layout.build.join(&target_bin);
        // We strip the library prefix before copying
        // target_bin is something like src/Test.ibc
        // we want to move build/src/Test.ibc to lib/Test.ibc
        let to = layout
            .lib
            .join(&target_bin.strip_prefix(source.path()).unwrap());

        fs::create_dir_all(to.parent().unwrap())?;
        fs::rename(from, to)?;
    }

    Ok(())
}

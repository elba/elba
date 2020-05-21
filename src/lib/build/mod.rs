//! Actually building Idris packages.

pub mod context;
pub mod invoke;
pub mod job;

use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use console::style;
use failure::{bail, format_err, ResultExt};
use futures::future;
use itertools::Itertools;
use rand::{distributions::Alphanumeric, seq::SliceRandom, thread_rng, Rng};
use walkdir::WalkDir;

use self::{
    context::BuildContext,
    invoke::{invoke_codegen, invoke_compile},
};
use crate::{
    retrieve::cache::{Binary, OutputLayout, Source},
    util::{
        clear_dir, copy_dir, copy_dir_iter,
        error::Result,
        fmt_multiple, fmt_output, generate_ipkg,
        shell::{OutputGroup, Shell, Verbosity},
        valid_file,
    },
};

/// A type of Target that should be built
#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Debug, Eq, Hash)]
pub enum Target {
    /// Build a library; the bool field is whether we should use codegen too
    Lib(bool),
    /// Compile a standalone executable which doesn't require the package's lib to be
    /// built
    ///
    /// The usize field is the index of the BinTarget in the manifest's list of BinTargets which
    /// should be built
    Bin(usize),
    // Test is like Bin, except that it requires the lib to be built already.
    Test(usize),
    // I would assume creating documentation requires the lib to be built too
    /// Create documentation
    Doc,
}

// This exists for the sake of hashing
impl Target {
    pub fn as_bytes(&self) -> [u8; 5] {
        match self {
            Target::Lib(x) => [0, 0, 0, 0, *x as u8],
            Target::Doc => [1, 0, 0, 0, 0],
            Target::Bin(x) => {
                let x = *x as u32;
                let b1: u8 = ((x >> 24) & 0xff) as u8;
                let b2: u8 = ((x >> 16) & 0xff) as u8;
                let b3: u8 = ((x >> 8) & 0xff) as u8;
                let b4: u8 = (x & 0xff) as u8;
                [2, b1, b2, b3, b4]
            }
            Target::Test(x) => {
                let x = *x as u32;
                let b1: u8 = ((x >> 24) & 0xff) as u8;
                let b2: u8 = ((x >> 16) & 0xff) as u8;
                let b3: u8 = ((x >> 8) & 0xff) as u8;
                let b4: u8 = (x & 0xff) as u8;
                [3, b1, b2, b3, b4]
            }
        }
    }

    pub fn is_codegen(&self) -> bool {
        match self {
            Target::Bin(_) | Target::Test(_) => true,
            _ => false,
        }
    }
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
                Target::Lib(_) => {
                    if !seen_lib {
                        res.push(i);
                        seen_lib = true;
                    }
                }
                Target::Bin(_) => {
                    res.push(i);
                }
                Target::Test(_) => {
                    res.push(i);
                }
                Target::Doc => {
                    if !seen_lib {
                        seen_lib = true;
                        res.insert(0, Target::Lib(false));
                    }
                    res.push(i);
                }
            }
        }

        Targets(res)
    }

    pub fn has_lib(&self) -> bool {
        self.0
            .get(0)
            .map(|x| if let Target::Lib(_) = x { true } else { false })
            .unwrap_or(false)
    }

    // This makes doc targets part of the
    pub fn is_codegen(&self) -> bool {
        self.0.iter().any(|x| x.is_codegen())
    }
}

pub async fn compile_lib<'a>(
    source: &'a Source,
    codegen: bool,
    deps: &'a [Binary],
    layout: &'a OutputLayout,
    bcx: &'a BuildContext,
    shell: Shell,
) -> Result<OutputGroup> {
    let lib_target = source.meta().targets.lib.clone().ok_or_else(|| {
        format_err!(
            "package {} doesn't contain a lib target",
            source.meta().package.name
        )
    })?;

    // We know that lib_target.path will be relative to the package root
    let src_path = source.path().join(&lib_target.path.0);
    let mut targets = lib_target
        .mods
        .iter()
        .map(|mod_name| {
            let path: PathBuf = mod_name.trim_matches('.').replace(".", "/").into();
            if src_path.join(&path).with_extension("idr").exists() {
                Ok(path.with_extension("idr"))
            } else if src_path.join(&path).with_extension("lidr").exists() {
                Ok(path.with_extension("lidr"))
            } else {
                Err(format_err!(
                    "module {} isn't a subpath and doesn't exist under path {}",
                    mod_name,
                    src_path.display()
                ))
            }
        })
        .collect::<Result<Vec<_>>>()?;

    let mut args = vec![];
    args.extend(lib_target.idris_opts.iter().map(|x| x.to_owned()));
    args.extend(bcx.opts.iter().cloned());

    let src_walker = source
        .meta()
        .list_files(source.path(), &src_path, |x| x.path() != layout.build)?;

    clear_dir(&layout.build.join("lib"))?;
    copy_dir_iter(src_walker, &src_path, &layout.build.join("lib"))?;

    run_prebuild_script(source, &layout.build.join("lib"), shell)?;

    // shuffle the targets to decreases the prosiblity that complier
    // overloads because of paralleling
    {
        let mut rng = thread_rng();
        targets.shuffle(&mut rng);
    }

    let mut compilations = targets.iter().map(|target| {
        let module = target
            .with_extension("")
            .to_string_lossy()
            .replace("/", ".")
            .replace("\\", ".");

        shell.println(
            style("Compiling").cyan(),
            format!("{} [{}]", module, source.meta().name()),
            Verbosity::Normal,
        );

        invoke_compile(deps, &target, layout.build.join("lib"), &args, bcx, shell)
    });

    let mut outputs = Vec::new();

    // Invoke compilations in parallel
    let mut ongoing_compilation = Vec::new();
    loop {
        while let Some(compile) = compilations.next() {
            ongoing_compilation.push(Box::pin(compile));
            if ongoing_compilation.len() >= bcx.threads as usize {
                break;
            }
        }

        if ongoing_compilation.is_empty() {
            break;
        }

        let (output, _, remaining) = future::select_all(ongoing_compilation).await;
        match output {
            Ok(output) => outputs.push(output),
            Err(err) => {
                bail!(err);
            }
        }
        ongoing_compilation = remaining;
    }

    let mut res = OutputGroup(outputs);

    clear_dir(&layout.lib)?;

    let from = if bcx.compiler.flavor().is_idris2() {
        layout.build.join("lib/build")
    } else {
        layout.build.join("lib")
    };

    let build_walker = WalkDir::new(&from).into_iter().filter_map(|x| {
        x.ok().and_then(|x| {
            if valid_file(&x)
                && x.path().extension() != Some(OsStr::new("idr"))
                && x.path().extension() != Some(OsStr::new("lidr"))
            {
                Some(x)
            } else {
                None
            }
        })
    });

    let lib_files = build_walker.collect::<Vec<_>>();

    clear_dir(&layout.lib)?;
    copy_dir_iter(lib_files.clone().into_iter(), &from, &layout.lib)?;

    if codegen {
        clear_dir(&layout.artifacts.join(&bcx.backend.name))?;

        // TODO: Idris 2 now doesn't support lib interface,
        // so it's temporarily safe to assume all the codegen
        // targets are ibc files.
        let lib_bins = lib_files
            .into_iter()
            .map(|x| x.into_path())
            .filter(|x| x.extension() == Some(OsStr::new("ibc")))
            .collect::<Vec<_>>();

        let output = invoke_codegen(
            &lib_bins,
            source.meta().name().name(),
            layout.build.join("lib"),
            layout.artifacts.join(&bcx.backend.name),
            true,
            &args,
            &bcx,
            shell,
        )
        .await?;

        res.push(output);
    }

    Ok(res)
}

pub async fn compile_bin<'a>(
    source: &'a Source,
    target: Target,
    deps: &'a [Binary],
    layout: &'a OutputLayout,
    bcx: &'a BuildContext,
    shell: Shell,
) -> Result<(OutputGroup, Option<PathBuf>)> {
    if bcx.compiler.flavor().is_idris2() {
        bail!("The Idris 2 compiler currently can't build executables")
    }

    let bin_target = match target {
        Target::Bin(ix) => source.meta().targets.bin[ix].clone(),
        Target::Test(ix) => source.meta().targets.test[ix].clone().into(),
        _ => bail!("compile_bin called with non-binary target"),
    };

    let (src_path, target_path) = bin_target.resolve_bin(source.path()).ok_or_else(|| {
        format_err!(
            "module {} isn't a subpath and doesn't exist under path {}",
            bin_target.main,
            bin_target.path.0.display()
        )
    })?;

    clear_dir(&layout.build.join("bin"))?;
    copy_dir(&src_path, &layout.build.join("bin"), false)?;

    run_prebuild_script(source, &layout.build.join("bin"), shell)?;

    // The Idris compiler ignores and rebuilds the imported
    // ibc modules if there are idrs match the modules name in
    // the source directory. So we copy the ibcs into the build
    // directory in advance to avoid that.
    if let Some(lib_target) = &source.meta().targets.lib {
        if lib_target.path == bin_target.path {
            copy_dir(&layout.lib, &layout.build.join("bin"), false)?;
        }
    }

    // Check extension etc
    let target_path = if let Some(ext) = target_path.extension() {
        if ext != OsStr::new("idr") && ext != OsStr::new("lidr") {
            let mod_name = &*target_path
                .with_extension("")
                .to_string_lossy()
                .replace("/", ".")
                .replace("\\", ".");
            make_main_file(mod_name, &*ext.to_string_lossy(), &layout.build.join("bin"))?
        } else {
            target_path
        }
    } else {
        target_path
    };

    let mut args = vec![];
    args.extend(bin_target.idris_opts.iter().map(|x| x.to_owned()));
    args.extend(bcx.opts.iter().cloned());

    let module = if target_path.is_absolute() {
        target_path
            .file_stem()
            .expect("bin target should be a file")
            .to_string_lossy()
            .to_string()
    } else {
        target_path
            .with_extension("")
            .to_string_lossy()
            .replace("/", ".")
            .replace("\\", ".")
    };
    shell.println(
        style("Compiling").cyan(),
        format!("{} [{}]", module, source.meta().name()),
        Verbosity::Normal,
    );

    let output = invoke_compile(
        deps,
        &target_path,
        layout.build.join("bin"),
        &args,
        bcx,
        shell,
    )
    .await?;

    let mut res = OutputGroup::from(output);

    let target_bin = target_path.with_extension("ibc");

    let name = if let Some(ex) = &bcx.backend.extension {
        let p: PathBuf = bin_target.name.into();
        p.with_extension(ex)
    } else {
        bin_target.name.into()
    };

    if !bcx.codegen {
        return Ok((res, None));
    }

    let binarys = &[layout.build.join("bin").join(&target_bin)];
    let output_name = &*name.to_string_lossy();

    shell.println(
        style("Codegen").cyan(),
        format!("{} ({}) [{}]", module, output_name, source.meta().name()),
        Verbosity::Normal,
    );

    let output = invoke_codegen(
        binarys,
        output_name,
        layout.build.join("bin"),
        layout.bin.clone(),
        false,
        &args,
        &bcx,
        shell,
    )
    .await?;

    // The output exectable will always go in target/bin
    res.push(output);

    let out = layout.bin.join(name);

    if out.exists() {
        Ok((res, Some(out)))
    } else if out.with_extension("exe").exists() {
        Ok((res, Some(out.with_extension("exe"))))
    } else {
        bail!("couldn't locate codegen output file: {}", out.display())
    }
}

pub async fn compile_doc<'a>(
    source: &'a Source,
    deps: &'a [Binary],
    layout: &'a OutputLayout,
    bcx: &'a BuildContext,
) -> Result<OutputGroup> {
    if bcx.compiler.flavor().is_idris2() {
        bail!("The Idris 2 compiler currently can't build documentation")
    }

    let lib_target = source.meta().targets.lib.clone().ok_or_else(|| {
        format_err!(
            "package {} doesn't contain a lib target, which is needed to build docs",
            source.meta().name()
        )
    })?;

    // If we're compiling docs, we assume that we've already built the lib

    // If we're just running a "check" command, we should never build docs
    if !bcx.codegen {
        return Ok(OutputGroup::new());
    }

    // Generate IPKG file
    let name = source.meta().name().name();
    let lib_path = "lib";
    let mut opts = String::new();
    let mods = lib_target.mods.join(", ");

    // Include dependencies
    for binary in deps {
        // We assume that the binary has already been compiled
        opts.push_str(format!("-i {}", &*binary.target.path().to_string_lossy()).as_ref());
    }

    opts.push_str(bcx.opts.iter().join(" ").as_str());

    let ipkg = generate_ipkg(&name, lib_path, &opts, &mods);

    fs::write(layout.build.join(".ipkg"), ipkg.as_bytes())
        .with_context(|e| format_err!("couldn't create temporary .ipkg file:\n{}", e))?;

    clear_dir(&layout.build.join("docs"))?;

    let mut process = bcx.compiler.process();
    process.current_dir(&layout.build);
    process.arg("--mkdoc").arg(".ipkg");

    let res = process.output()?;
    if !res.status.success() {
        bail!(
            "> {:#?}\n--- stdout\n{}\n--- stderr\n{}",
            process,
            String::from_utf8_lossy(&res.stdout),
            String::from_utf8_lossy(&res.stderr),
        )
    }

    fs::remove_file(layout.build.join(".ipkg"))
        .with_context(|e| format_err!("couldn't remove temporary .ipkg file:\n{}", e))?;

    clear_dir(&layout.docs)?;

    fs::rename(layout.build.join(format!("{}_doc", &name)), &layout.docs).with_context(|e| {
        format_err!(
            "docs located at {}_doc; couldn't move them to docs:\n{}",
            name,
            e
        )
    })?;

    Ok(res.into())
}

pub fn run_script(root: &Path, cmd: &str) -> Result<OutputGroup> {
    let mut process = if cfg!(target_os = "windows") {
        let mut p = Command::new("cmd");
        p.args(&["/C", cmd]);
        p
    } else {
        let mut p = Command::new("sh");
        p.arg("-c");
        p.arg(cmd);
        p
    };

    process.current_dir(root);

    if let Ok(v) = env::var("PATH") {
        process.env("PATH", v);
    }

    let res = process.output()?;
    if !res.status.success() {
        bail!("> {:#?}\n{}", process, fmt_output(&res))
    }

    Ok(res.into())
}

pub fn run_prebuild_script(source: &Source, root: &Path, shell: Shell) -> Result<()> {
    if let Some(s) = source.meta().scripts.get("prebuild") {
        shell.println(
            style("Running").dim(),
            format!("prebuild script > {}", s),
            Verbosity::Verbose,
        );
        shell.println_plain(fmt_multiple(&run_script(root, s)?), Verbosity::Normal);
    }

    Ok(())
}

fn make_main_file(module: &str, fun: &str, parent: &Path) -> Result<PathBuf> {
    let rstr: String = thread_rng().sample_iter(&Alphanumeric).take(8).collect();
    let fname = format!("elba-{}.idr", rstr);
    fs::write(
        parent.join(&fname),
        format!(
            r#"module Main

import {}

main : IO ()
main = {}"#,
            module, fun
        ),
    )?;
    Ok(parent.join(fname))
}

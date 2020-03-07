//! Utilities for interacting with the Idris compiler

use crate::{
    build::context::BuildContext,
    retrieve::cache::Binary,
    util::{
        error::Result,
        fmt_output,
        shell::{Shell, Verbosity},
    },
};
use failure::bail;
use futures::compat::Future01CompatExt;
use itertools::Itertools;
use std::{
    path::{Path, PathBuf},
    process::Output,
    time::{Duration, Instant},
};
use tokio::timer::Delay;
use tokio_process::CommandExt;

// dealing with ibc stuff
pub async fn invoke_compile<'a>(
    deps: &'a [Binary],
    target: &'a Path,
    build: PathBuf,
    args: &'a [String],
    bcx: &'a BuildContext,
    shell: Shell,
) -> Result<Output> {
    let mut process = bcx.compiler.process();
    let flavor = bcx.compiler.flavor();
    process.current_dir(&build).arg("--check");

    // Include dependencies
    if flavor.is_idris1() {
        for binary in deps {
            // We assume that the binary has already been compiled
            process.arg("-i").arg(binary.target.path());
        }
    } else {
        process.env(
            "BLODWEN_PATH",
            deps.iter()
                .map(|x| x.target.path().to_string_lossy())
                .join(":"),
        );
    }

    process.args(args);
    process.arg(target);

    // Idris sometimes breaks down if multiple modules is being compiled
    // in parallel. We reties if the stdout contains 'loadable'.
    for _ in 0..15usize {
        shell.println_plain(format!("> {:#?}", process), Verbosity::Verbose);

        let output = process.output_async().compat().await?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("loadable") {
                Delay::new(Instant::now() + Duration::from_secs(3))
                    .compat()
                    .await?;
            } else {
                bail!("> {:#?}\n{}", process, fmt_output(&output))
            }
        } else {
            return Ok(output);
        }
    }

    bail!("Compiler is overloaded on compiling: {}", target.display());
}

// If we want to create something with output from a codegen backend (either a library or a binary)
// we look to invoke_codegen.
pub async fn invoke_codegen<'a>(
    binary: &'a [PathBuf],
    output: &'a str,
    build: PathBuf,
    output_dir: PathBuf,
    // Whether the output should be treated as a binary (false) or artifact files (true)
    is_artifact: bool,
    args: &'a [String],
    bcx: &'a BuildContext,
    shell: Shell,
) -> Result<Output> {
    let mut process = bcx.compiler.process();
    let flavor = bcx.compiler.flavor();

    if is_artifact {
        if flavor.is_idris1() {
            process.arg("--interface");
        } else {
            bail!("Only the Idris 1 compiler supports library artifacts");
        }
    }

    process
        .current_dir(output_dir)
        .args(&["-o", &output])
        .args(&[
            if bcx.backend.portable && flavor.is_idris1() {
                "--portable-codegen"
            } else {
                "--codegen"
            },
            &bcx.backend.name,
        ]);

    if !bcx.backend.opts.is_empty() && flavor.is_idris1() {
        process
            .arg("--cg-opt")
            .arg(bcx.backend.opts.iter().join(" "));
    }

    process.args(args);

    if flavor.is_idris1() {
        process.arg("-i");
        process.arg(&build);
    } else {
        process.env(
            "BLODWEN_PATH",
            binary
                .iter()
                .map(|x| x.parent().unwrap().to_string_lossy())
                .join(":"),
        );
    }

    for bin in binary {
        process.arg(bin);
    }

    shell.println_plain(format!("> {:#?}", process), Verbosity::Verbose);

    let output = process.output_async().compat().await?;

    // The Idris compiler is stupid, and won't output a non-zero error code if there's no main
    // function in the file, so we manually check if stdout contains a "main not found" error.
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("No such variable Main.main") || !output.status.success() {
        bail!("> {:#?}\n{}", process, fmt_output(&output))
    }

    Ok(output)
}

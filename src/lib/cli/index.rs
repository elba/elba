//! Registry-related commands: publishing, yanking, etc.

use std::{
    fs::{create_dir_all, File},
    path::{Path, PathBuf},
    str::{self},
};

use flate2::{write::GzEncoder, Compression};
use tar;

use super::build;
use crate::{
    cli::build::find_manifest,
    package::manifest::Manifest,
    retrieve::Cache,
    util::{error::Result, valid_file},
};

pub fn package(project: &Path) -> Result<(PathBuf, Manifest)> {
    let (project, manifest) = find_manifest(project, false, None)?;

    let gz_name = format!(
        "target/{}_{}-{}.tar.gz",
        manifest.name().group(),
        manifest.name().name(),
        manifest.version()
    );

    create_dir_all(project.join("target"))?;
    let tar_gz = File::create(project.join(&gz_name))?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    let walker = manifest
        .list_files(&project, &project, |x| {
            x.file_name() != ".git" && x.file_name() != "target"
        })?
        .filter(valid_file);

    for item in walker {
        let suffix = item.path().strip_prefix(&project).unwrap();
        tar.append_path_with_name(item.path(), suffix)?;
    }

    // Finish writing to the tarball
    drop(tar);

    Ok((project.join(&gz_name), manifest))
}

pub fn search(bcx: &build::BuildCtx, query: &str) -> Result<String> {
    let cache = Cache::from_disk(&bcx.logger, bcx.global_cache.clone(), bcx.shell)?;
    let ixs = bcx
        .indices
        .values()
        .cloned()
        .map(|x| x.res)
        .collect::<Vec<_>>();
    let indices = cache.get_indices(&ixs, false, false);

    let pkgs = indices.search(query)?;
    let mut res = String::new();

    for (name, ver, ir) in &pkgs {
        if ir.res == ixs[0] {
            res.push_str(&format!("\"{}\" = \"{}\"", name, ver));
        } else {
            res.push_str(&format!(
                "\"{}\" = \"{{ version = {}, index = {} }}\"",
                name, ver, ir
            ));
        }
    }

    Ok(res)
}

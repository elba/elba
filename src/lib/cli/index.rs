//! Registry-related commands: publishing, yanking, etc.

use super::build;
use crate::{
    package::{manifest::Manifest, Name},
    remote::resolution::IndexRes,
    retrieve::Cache,
    util::{config, errors::Res, valid_file},
};
use failure::{format_err, ResultExt};
use flate2::{write::GzEncoder, Compression};
use semver::Version;
use std::{
    env::set_current_dir,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    str::{self, FromStr},
};
use tar;

pub struct RegistryCtx {
    pub index: IndexRes,
    pub data_dir: PathBuf,
}

pub fn package(
    ctx: &build::BuildCtx,
    project: &Path,
    verify: bool,
) -> Res<(PathBuf, Name, Version)> {
    if verify {
        build::build(
            &ctx,
            project,
            &(true, false, None, None),
            true,
            &config::Backend::default(),
        )?;
    }

    // build succeeded already, so we can unwrap ok!
    let nproj = build::find_manifest_root(&project).unwrap();

    let mut contents = String::new();
    let mut manifest = File::open(nproj.join("elba.toml"))
        .context(format_err!("failed to read manifest file (elba.toml)"))?;
    manifest.read_to_string(&mut contents)?;
    let manifest = Manifest::from_str(&contents)?;

    let gz_name = format!(
        "target/{}_{}-{}.tar.gz",
        manifest.name().group(),
        manifest.name().name(),
        manifest.version()
    );

    let tar_gz = File::create(nproj.join(&gz_name))?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    let walker = manifest
        .list_files(&nproj, &nproj, |x| {
            x.file_name() != ".git" && x.file_name() != "target"
        })?
        .filter(valid_file);

    set_current_dir(&nproj)?;

    for item in walker {
        let suffix = item.path().strip_prefix(&nproj).unwrap();
        tar.append_path(suffix)?;
    }

    // Finish writing to the tarball
    drop(tar);

    Ok((
        nproj.join(&gz_name),
        manifest.name().clone(),
        manifest.version().clone(),
    ))
}

pub fn search(bcx: &build::BuildCtx, query: &str) -> Res<String> {
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

//! Repository-related commands: publishing, yanking, etc.

use super::build;
use failure::ResultExt;
use flate2::{write::GzEncoder, Compression};
use package::manifest::Manifest;
use remote::{self, resolution::DirectRes};
use retrieve::Cache;
use std::{env::set_current_dir, fs::File, io::Read, path::Path, str::FromStr};
use tar;
use util::{config, errors::Res};
use walkdir::WalkDir;

pub fn package(ctx: &build::BuildCtx, project: &Path, verify: bool) -> Res<String> {
    if verify {
        build::build(
            &ctx,
            project,
            &(true, true, None, None),
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

    // TODO: More flexible ignores
    let walker = WalkDir::new(&nproj)
        .into_iter()
        .filter_entry(|x| x.file_name() != ".git" && x.file_name() != "target")
        .filter(|x| x.is_ok() && x.as_ref().unwrap().file_type().is_file());

    set_current_dir(&nproj)?;

    for item in walker {
        let item = item.unwrap();
        let suffix = item.path().strip_prefix(&nproj).unwrap();
        tar.append_path(suffix)?;
    }

    // Finish writing to the tarball
    drop(tar);

    Ok(format!("created compressed tarball at `{}`", gz_name))
}

fn get_backend(c: &mut Cache, d: DirectRes) -> Res<remote::Backend> {
    let dt = d.to_string();
    c.get_indices(&[d], false, false)
        .indices
        .get_index(0)
        .ok_or_else(|| format_err!("invalid index: {}", dt))?
        .1
        .backend()
        .ok_or_else(|| format_err!("index {} has no backend", dt))
        .map(|x| x.clone())
}

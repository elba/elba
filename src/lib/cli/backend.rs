//! Repository-related commands: publishing, yanking, etc.

use super::build;
use crate::{
    package::{manifest::Manifest, Name},
    remote::{
        self,
        resolution::{DirectRes, IndexRes},
    },
    retrieve::Cache,
    util::{config, errors::Res, valid_file},
};
use failure::{format_err, ResultExt};
use flate2::{write::GzEncoder, Compression};
use indexmap::IndexMap;
use semver::Version;
use std::{
    env::set_current_dir,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    str::{self, FromStr},
};
use tar;
use toml;

pub struct BackendCtx {
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

pub fn login(ctx: &BackendCtx, token: &str) -> Res<String> {
    let (mut logins, logins_file) = get_logins(ctx)?;
    logins.insert(ctx.index.clone(), token.to_owned());
    fs::write(&logins_file, toml::to_string(&logins).unwrap()).context(format_err!(
        "couldn't write to logins file {}",
        logins_file.display()
    ))?;
    Ok(format!("successfully logged into index {}", &ctx.index))
}

pub fn yank(bcx: &build::BuildCtx, ctx: &BackendCtx, name: &Name, ver: &Version) -> Res<()> {
    let token = get_token(ctx)?;
    let mut cache = Cache::from_disk(&bcx.logger, bcx.global_cache.clone(), bcx.shell)?;
    let backend = get_backend(&mut cache, ctx.index.res.clone())?;
    backend.yank(name, ver, &token)?;
    Ok(())
}

pub fn publish(bcx: &build::BuildCtx, ctx: &BackendCtx, project: &Path, verify: bool) -> Res<()> {
    let token = get_token(ctx)?;
    let mut cache = Cache::from_disk(&bcx.logger, bcx.global_cache.clone(), bcx.shell)?;
    let backend = get_backend(&mut cache, ctx.index.res.clone())?;

    let (tar, name, ver) = package(bcx, project, verify)?;
    let tar = File::open(tar)?;

    backend.publish(tar, &name, &ver, &token)?;

    Ok(())
}

fn get_logins(ctx: &BackendCtx) -> Res<(IndexMap<IndexRes, String>, PathBuf)> {
    let logins_file = ctx.data_dir.join("logins.toml");

    if !logins_file.exists() {
        File::create(&logins_file).with_context(|e| {
            format_err!(
                "couldn't create logins file {}: {}",
                logins_file.display(),
                e
            )
        })?;
    }

    let logins = fs::read(&logins_file).with_context(|e| {
        format_err!("couldn't open logins file {}: {}", logins_file.display(), e)
    })?;
    let logins: IndexMap<IndexRes, String> = toml::from_slice(&logins).with_context(|e| {
        format_err!("invalid logins format for {}: {}", logins_file.display(), e)
    })?;

    Ok((logins, logins_file))
}

fn get_token(ctx: &BackendCtx) -> Res<String> {
    let (logins, logins_file) = get_logins(&ctx)?;
    logins.get(&ctx.index).cloned().ok_or_else(|| {
        format_err!(
            "login for {} not found in logins file {}",
            &ctx.index,
            logins_file.display()
        )
    })
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

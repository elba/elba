//! Registry-related commands: publishing, yanking, etc.

use super::build;
use crate::{
    package::{manifest::Manifest, Name},
    remote::{
        self,
        resolution::{DirectRes, IndexRes},
    },
    retrieve::Cache,
    util::{config, errors::Res, shell::Verbosity, valid_file},
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

pub fn login(ctx: &RegistryCtx, token: &str) -> Res<String> {
    let (mut logins, logins_file) = get_logins(ctx)?;
    logins.insert(ctx.index.clone(), token.to_owned());
    fs::write(&logins_file, toml::to_string(&logins).unwrap()).context(format_err!(
        "couldn't write to logins file {}",
        logins_file.display()
    ))?;
    Ok(format!("successfully logged into index {}", &ctx.index))
}

pub fn yank(
    bcx: &build::BuildCtx,
    ctx: &RegistryCtx,
    name: &Name,
    ver: &Version,
    yank: bool,
) -> Res<()> {
    let token = get_token(ctx)?;
    let mut cache = Cache::from_disk(&bcx.logger, bcx.global_cache.clone(), bcx.shell)?;
    let registry = get_registry(&mut cache, ctx.index.res.clone(), false).1?;
    registry.yank(name, ver, &token, yank)?;
    Ok(())
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

pub fn publish(bcx: &build::BuildCtx, ctx: &RegistryCtx, project: &Path, verify: bool) -> Res<()> {
    let token = get_token(ctx)?;
    let mut cache = Cache::from_disk(&bcx.logger, bcx.global_cache.clone(), bcx.shell)?;
    let registry = get_registry(&mut cache, ctx.index.res.clone(), true).1?;

    let (tar, name, ver) = package(bcx, project, verify)?;
    let tar = File::open(tar)?;

    bcx.shell.println(
        "Publishing",
        format!("package {}|{}", name, ver),
        Verbosity::Normal,
    );
    registry.publish(tar, &name, &ver, &token)?;

    Ok(())
}

fn get_logins(ctx: &RegistryCtx) -> Res<(IndexMap<IndexRes, String>, PathBuf)> {
    let logins_file = ctx.data_dir.join("logins.toml");

    if !logins_file.exists() {
        fs::create_dir_all(&ctx.data_dir)?;
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

fn get_token(ctx: &RegistryCtx) -> Res<String> {
    let (logins, logins_file) = get_logins(&ctx)?;
    logins.get(&ctx.index).cloned().ok_or_else(|| {
        format_err!(
            "login for {} not found in logins file {}",
            &ctx.index,
            logins_file.display()
        )
    })
}

fn get_registry(
    c: &mut Cache,
    d: DirectRes,
    eager: bool,
) -> (remote::Indices, Res<remote::Registry>) {
    let dt = d.to_string();
    let indices = c.get_indices(&[d], eager, false);
    let registry = (|| {
        indices
            .indices
            .get_index(0)
            .ok_or_else(|| format_err!("invalid index: {}", dt))?
            .1
            .registry()
            .ok_or_else(|| format_err!("index {} has no registry", dt))
            .map(|x| x.clone())
    })();

    (indices, registry)
}

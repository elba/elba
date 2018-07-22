use failure::ResultExt;
use std::{fs, io::prelude::*};
use util::errors::Res;

pub struct BuildCtx {
    pub manifest: PathBuf,
    pub global_cache: PathBuf,
}

pub fn lock(ctx: &BuildCtx) -> Res<Cache> {
    let manifest = fs::File::open(ctx.manifest).context(format_err!("failed to read manifest file."))?;
    let mut contents = String::new();
    manifest.read_to_string(&mut contents);

    let manifest = Manifest::from_str(&contents)?;

    // TODO: Get indices from config & cache.
    unimplemented!()
}
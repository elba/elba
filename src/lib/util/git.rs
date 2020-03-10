//! Utilities for dealing with git repos.

// Much of this code is taken from Cargo's git utilities, which are available are the following link:
// https://github.com/rust-lang/cargo/blob/master/src/cargo/sources/git/utils.rs
// As such, the MIT license under which Cargo is licensed is provided in full:
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use crate::util::error::Result;
use failure::{format_err, ResultExt};
use git2;
use std::{env, fs, path::Path};
use url::Url;

pub fn init(path: &Path) -> Result<()> {
    git2::Repository::discover(path).or_else(|_| git2::Repository::init(path))?;
    Ok(())
}

pub fn clone(url: &Url, into: &Path) -> Result<git2::Repository> {
    let git_config = git2::Config::open_default()?;
    with_fetch_options(&git_config, &url, &mut |opts| {
        let repo = git2::build::RepoBuilder::new()
            .fetch_options(opts)
            .clone(url.as_str(), into)?;

        Ok(repo)
    })
}

pub fn update_submodules(repo: &git2::Repository) -> Result<()> {
    for mut child in repo.submodules()? {
        update_submodule(repo, &mut child).with_context(|_| {
            format!(
                "failed to update submodule `{}`",
                child.name().unwrap_or("")
            )
        })?;
    }
    Ok(())
}

fn update_submodule(parent: &git2::Repository, child: &mut git2::Submodule) -> Result<()> {
    child.init(false)?;
    let url = child
        .url()
        .ok_or_else(|| format_err!("non-utf8 url for submodule"))?;

    // A submodule which is listed in .gitmodules but not actually
    // checked out will not have a head id, so we should ignore it.
    let head = match child.head_id() {
        Some(head) => head,
        None => return Ok(()),
    };

    // If the submodule hasn't been checked out yet, we need to
    // clone it. If it has been checked out and the head is the same
    // as the submodule's head, then we can skip an update and keep
    // recursing.
    let head_and_repo = child.open().and_then(|repo| {
        let target = repo.head()?.target();
        Ok((target, repo))
    });
    let mut repo = match head_and_repo {
        Ok((head, repo)) => {
            if child.head_id() == head {
                return update_submodules(&repo);
            }
            repo
        }
        Err(..) => {
            let path = parent.workdir().unwrap().join(child.path());
            let _ = remove_dir_all::remove_dir_all(&path);
            git2::Repository::init(&path)?
        }
    };

    // Fetch data from origin and reset to the head commit
    let refspec = "refs/heads/*:refs/heads/*";
    let url = Url::parse(url)?;
    fetch(&mut repo, &url, refspec).with_context(|_| {
        format_err!(
            "failed to fetch submodule `{}` from {}",
            child.name().unwrap_or(""),
            url
        )
    })?;

    let obj = repo.find_object(head, None)?;
    reset(&repo, &obj)?;
    update_submodules(&repo)
}

pub fn fetch(repo: &mut git2::Repository, url: &Url, refspec: &str) -> Result<()> {
    // The `fetch` operation here may fail spuriously due to a corrupt
    // repository. It could also fail, however, for a whole slew of other
    // reasons (aka network related reasons). We want Cargo to automatically
    // recover from corrupt repositories, but we don't want Cargo to stomp
    // over other legitimate errors.o
    //
    // Consequently we save off the error of the `fetch` operation and if it
    // looks like a "corrupt repo" error then we blow away the repo and try
    // again. If it looks like any other kind of error, or if we've already
    // blown away the repository, then we want to return the error as-is.
    let mut repo_reinitialized = false;
    let git_config = git2::Config::open_default()?;
    with_fetch_options(&git_config, url, &mut |mut opts| {
        loop {
            let res = repo
                .remote_anonymous(url.as_str())?
                .fetch(&[refspec], Some(&mut opts), None);
            let err = match res {
                Ok(()) => break,
                Err(e) => e,
            };

            if !repo_reinitialized && err.class() == git2::ErrorClass::Reference {
                repo_reinitialized = true;
                // This is a corrupt repo; reinit and try again
                if reinitialize(repo).is_ok() {
                    continue;
                }
            }

            return Err(err.into());
        }

        Ok(())
    })
}

pub fn reset(repo: &git2::Repository, obj: &git2::Object) -> Result<()> {
    let mut opts = git2::build::CheckoutBuilder::new();
    repo.reset(obj, git2::ResetType::Hard, Some(&mut opts))?;
    Ok(())
}

fn reinitialize(repo: &mut git2::Repository) -> Result<()> {
    // Here we want to drop the current repository object pointed to by `repo`,
    // so we initialize temporary repository in a sub-folder, blow away the
    // existing git folder, and then recreate the git repo. Finally we blow away
    // the `tmp` folder we allocated.
    let path = repo.path().to_path_buf();
    let tmp = path.join("tmp");
    let bare = !repo.path().ends_with(".git");
    *repo = git2::Repository::init(&tmp)?;
    for entry in path.read_dir()? {
        let entry = entry?;
        if entry.file_name().to_str() == Some("tmp") {
            continue;
        }
        let path = entry.path();
        drop(fs::remove_file(&path).or_else(|_| remove_dir_all::remove_dir_all(&path)));
    }
    if bare {
        *repo = git2::Repository::init_bare(path)?;
    } else {
        *repo = git2::Repository::init(path)?;
    }
    remove_dir_all::remove_dir_all(&tmp)?;
    Ok(())
}

fn with_authentication<T, F>(url: &str, cfg: &git2::Config, mut f: F) -> Result<T>
where
    F: FnMut(&mut git2::Credentials) -> Result<T>,
{
    let mut cred_helper = git2::CredentialHelper::new(url);
    cred_helper.config(cfg);

    let mut ssh_username_requested = false;
    let mut cred_helper_bad = None;
    let mut ssh_agent_attempts = Vec::new();
    let mut any_attempts = false;
    let mut tried_sshkey = false;

    let mut res = f(&mut |url, username, allowed| {
        any_attempts = true;
        // libgit2's "USERNAME" authentication actually means that it's just
        // asking us for a username to keep going. This is currently only really
        // used for SSH authentication and isn't really an authentication type.
        // The logic currently looks like:
        //
        //      let user = ...;
        //      if (user.is_null())
        //          user = callback(USERNAME, null, ...);
        //
        //      callback(SSH_KEY, user, ...)
        //
        // So if we're being called here then we know that (a) we're using ssh
        // authentication and (b) no username was specified in the URL that
        // we're trying to clone. We need to guess an appropriate username here,
        // but that may involve a few attempts. Unfortunately we can't switch
        // usernames during one authentication session with libgit2, so to
        // handle this we bail out of this authentication session after setting
        // the flag `ssh_username_requested`, and then we handle this below.
        if allowed.contains(git2::CredentialType::USERNAME) {
            debug_assert!(username.is_none());
            ssh_username_requested = true;
            return Err(git2::Error::from_str("gonna try usernames later"));
        }

        // An "SSH_KEY" authentication indicates that we need some sort of SSH
        // authentication. This can currently either come from the ssh-agent
        // process or from a raw in-memory SSH key. Cargo only supports using
        // ssh-agent currently.
        //
        // If we get called with this then the only way that should be possible
        // is if a username is specified in the URL itself (e.g. `username` is
        // Some), hence the unwrap() here. We try custom usernames down below.
        if allowed.contains(git2::CredentialType::SSH_KEY) && !tried_sshkey {
            // If ssh-agent authentication fails, libgit2 will keep
            // calling this callback asking for other authentication
            // methods to try. Make sure we only try ssh-agent once,
            // to avoid looping forever.
            tried_sshkey = true;
            let username = username.unwrap();
            debug_assert!(!ssh_username_requested);
            ssh_agent_attempts.push(username.to_string());
            return git2::Cred::ssh_key_from_agent(username);
        }

        // Sometimes libgit2 will ask for a username/password in plaintext. This
        // is where Cargo would have an interactive prompt if we supported it,
        // but we currently don't! Right now the only way we support fetching a
        // plaintext password is through the `credential.helper` support, so
        // fetch that here.
        if allowed.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            let r = git2::Cred::credential_helper(cfg, url, username);
            cred_helper_bad = Some(r.is_err());
            return r;
        }

        // I'm... not sure what the DEFAULT kind of authentication is, but seems
        // easy to support?
        if allowed.contains(git2::CredentialType::DEFAULT) {
            return git2::Cred::default();
        }

        // Welp, we tried our best
        Err(git2::Error::from_str("no authentication available"))
    });

    // Ok, so if it looks like we're going to be doing ssh authentication, we
    // want to try a few different usernames as one wasn't specified in the URL
    // for us to use. In order, we'll try:
    //
    // * A credential helper's username for this URL, if available.
    // * This account's username.
    // * "git"
    //
    // We have to restart the authentication session each time (due to
    // constraints in libssh2 I guess? maybe this is inherent to ssh?), so we
    // call our callback, `f`, in a loop here.
    if ssh_username_requested {
        debug_assert!(res.is_err());
        let mut attempts = Vec::new();
        attempts.push("git".to_string());
        if let Ok(s) = env::var("USER").or_else(|_| env::var("USERNAME")) {
            attempts.push(s);
        }
        if let Some(ref s) = cred_helper.username {
            attempts.push(s.clone());
        }

        while let Some(s) = attempts.pop() {
            // We should get `USERNAME` first, where we just return our attempt,
            // and then after that we should get `SSH_KEY`. If the first attempt
            // fails we'll get called again, but we don't have another option so
            // we bail out.
            let mut attempts = 0;
            res = f(&mut |_url, username, allowed| {
                if allowed.contains(git2::CredentialType::USERNAME) {
                    return git2::Cred::username(&s);
                }
                if allowed.contains(git2::CredentialType::SSH_KEY) {
                    debug_assert_eq!(Some(&s[..]), username);
                    attempts += 1;
                    if attempts == 1 {
                        ssh_agent_attempts.push(s.to_string());
                        return git2::Cred::ssh_key_from_agent(&s);
                    }
                }
                Err(git2::Error::from_str("no authentication available"))
            });

            // If we made two attempts then that means:
            //
            // 1. A username was requested, we returned `s`.
            // 2. An ssh key was requested, we returned to look up `s` in the
            //    ssh agent.
            // 3. For whatever reason that lookup failed, so we were asked again
            //    for another mode of authentication.
            //
            // Essentially, if `attempts == 2` then in theory the only error was
            // that this username failed to authenticate (e.g. no other network
            // errors happened). Otherwise something else is funny so we bail
            // out.
            if attempts != 2 {
                break;
            }
        }
    }

    if res.is_ok() || !any_attempts {
        return res.map_err(From::from);
    }

    // In the case of an authentication failure (where we tried something) then
    // we try to give a more helpful error message about precisely what we
    // tried.
    let res = res.with_context(|_| {
        let mut msg = "failed to authenticate when downloading \
                       repository"
            .to_string();
        if !ssh_agent_attempts.is_empty() {
            let names = ssh_agent_attempts
                .iter()
                .map(|s| format!("`{}`", s))
                .collect::<Vec<_>>()
                .join(", ");
            msg.push_str(&format!(
                "\nattempted ssh-agent authentication, but \
                 none of the usernames {} succeeded",
                names
            ));
        }
        if let Some(failed_cred_helper) = cred_helper_bad {
            if failed_cred_helper {
                msg.push_str(
                    "\nattempted to find username/password via \
                     git's `credential.helper` support, but failed",
                );
            } else {
                msg.push_str(
                    "\nattempted to find username/password via \
                     `credential.helper`, but maybe the found \
                     credentials were incorrect",
                );
            }
        }
        msg
    })?;
    Ok(res)
}

pub fn with_fetch_options<T>(
    git_config: &git2::Config,
    url: &Url,
    cb: &mut dyn FnMut(git2::FetchOptions) -> Result<T>,
) -> Result<T> {
    with_authentication(url.as_str(), git_config, |f| {
        let mut rcb = git2::RemoteCallbacks::new();
        rcb.credentials(f);

        // Create a local anonymous remote in the repository to fetch the
        // url
        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(rcb)
            .download_tags(git2::AutotagOption::All);
        cb(opts)
    })
}

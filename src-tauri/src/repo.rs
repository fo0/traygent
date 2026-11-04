//! GitHub repositories as a start target (docs/SPEC.md → Repositories, docs/adr/0005): what
//! the folder field's text means, the rules for `owner/name` and where a clone lives.
use crate::detect::Host;
use crate::launch::normalize_folder;
use crate::models::RepoRef;

const MAX_OWNER_CHARS: usize = 39;
const MAX_NAME_CHARS: usize = 100;
pub const INVALID_OWNER: &str = "Ungültiger GitHub-Besitzer im Repository.";
pub const INVALID_NAME: &str = "Ungültiger Repository-Name.";

/// What the folder field's trimmed text names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Empty,
    /// An absolute Windows path, as typed (trimmed); validated by the launch.
    Folder(String),
    Repo(RepoRef),
    /// Search text for the account's repositories (trimmed).
    Search(String),
}

/// 1–39 of `A-Za-z0-9-`, no leading or trailing `-`.
pub fn is_valid_owner(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= MAX_OWNER_CHARS
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-')
}

/// 1–100 of `A-Za-z0-9._-`, not `.` or `..` (after the `.git` suffix is dropped).
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_CHARS
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && name != "."
        && name != ".."
}

/// The repository with one `.git` suffix dropped from its name, or a German error.
pub fn validate(repo: &RepoRef) -> Result<RepoRef, String> {
    let name = repo.name.strip_suffix(".git").unwrap_or(&repo.name);
    if !is_valid_owner(&repo.owner) {
        return Err(INVALID_OWNER.into());
    }
    if !is_valid_name(name) {
        return Err(INVALID_NAME.into());
    }
    Ok(RepoRef {
        owner: repo.owner.clone(),
        name: name.into(),
    })
}

fn repo(owner: &str, name: &str) -> Option<RepoRef> {
    validate(&RepoRef {
        owner: owner.into(),
        name: name.into(),
    })
    .ok()
}

/// Strips an ASCII prefix, ignoring case.
fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// `[http[s]://][www.]github.com/<owner>/<name>[.git][/…][?…][#…]`, without whitespace.
fn from_url(text: &str) -> Option<RepoRef> {
    if text.contains(char::is_whitespace) {
        return None;
    }
    let rest = strip_prefix_ci(text, "https://")
        .or_else(|| strip_prefix_ci(text, "http://"))
        .unwrap_or(text);
    let rest = strip_prefix_ci(rest, "www.").unwrap_or(rest);
    let path = strip_prefix_ci(rest, "github.com/")?;
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let mut segments = path.split('/');
    repo(segments.next()?, segments.next()?)
}

/// `git@github.com:<owner>/<name>[.git]`.
fn from_ssh(text: &str) -> Option<RepoRef> {
    let path = strip_prefix_ci(text, "git@github.com:")?;
    let (owner, name) = path.split_once('/')?;
    repo(owner, name)
}

/// `<owner>/<name>[.git]`, exactly one `/`.
fn from_slug(text: &str) -> Option<RepoRef> {
    let (owner, name) = text.split_once('/')?;
    repo(owner, name)
}

/// Classifies the folder field's text exactly as the panel does (`fixtures/targets.json`).
pub fn classify(input: &str) -> Target {
    let text = input.trim();
    if text.is_empty() {
        return Target::Empty;
    }
    if crate::launch::is_windows_absolute(text) {
        return Target::Folder(text.into());
    }
    from_url(text)
        .or_else(|| from_ssh(text))
        .or_else(|| from_slug(text))
        .map_or_else(|| Target::Search(text.into()), Target::Repo)
}

/// What `git clone` fetches; the user's own Git credentials apply, never Traygent's token.
pub fn clone_url(repo: &RepoRef) -> String {
    format!("https://github.com/{}/{}.git", repo.owner, repo.name)
}

/// `%USERPROFILE%\Traygent`, normalized; `''` when the profile is unset or no valid
/// absolute Windows folder.
pub fn default_clone_root(host: &Host) -> String {
    host.user_profile
        .as_deref()
        .and_then(|profile| normalize_folder(&format!(r"{profile}\Traygent")).ok())
        .unwrap_or_default()
}

/// `<root>\<owner>\<name>` for an already validated repository, validated like any folder.
pub fn clone_folder(root: &str, repo: &RepoRef) -> Result<String, String> {
    normalize_folder(&format!(r"{root}\{}\{}", repo.owner, repo.name))
}

#[cfg(test)]
mod tests;

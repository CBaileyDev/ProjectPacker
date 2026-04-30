use crate::error::{CoreError, CoreResult};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGithubUrl {
    pub owner: String,
    pub repo: String,
    pub https_url: String,
}

pub fn parse_github_url(url: &str) -> CoreResult<ParsedGithubUrl> {
    let s = url.trim().trim_end_matches('/');

    let path = if let Some(rest) = s.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = s.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = s.strip_prefix("github.com/") {
        rest
    } else {
        return Err(CoreError::InvalidTarget(format!("not a github url: {url}")));
    };

    let path = path.trim_end_matches(".git");
    let mut parts = path.splitn(3, '/');
    let owner = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CoreError::InvalidTarget(format!("missing owner: {url}")))?;
    let repo = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CoreError::InvalidTarget(format!("missing repo: {url}")))?;

    Ok(ParsedGithubUrl {
        owner: owner.to_string(),
        repo: repo.to_string(),
        https_url: format!("https://github.com/{owner}/{repo}.git"),
    })
}

pub struct ClonedRepo {
    pub path: PathBuf,
    _guard: tempfile::TempDir,
}

pub fn shallow_clone(url: &str, job_id: &str) -> CoreResult<ClonedRepo> {
    let parsed = parse_github_url(url)?;
    let temp = tempfile::Builder::new()
        .prefix(&format!("projectpacker-{job_id}-"))
        .tempdir()
        .map_err(|e| CoreError::CloneFailed(format!("temp dir: {e}")))?;
    let target = temp.path().join(&parsed.repo);

    gix::prepare_clone(parsed.https_url.as_str(), &target)
        .map_err(|e| CoreError::CloneFailed(e.to_string()))?
        .with_shallow(gix::remote::fetch::Shallow::DepthAtRemote(
            std::num::NonZeroU32::new(1).unwrap(),
        ))
        .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|e| CoreError::CloneFailed(e.to_string()))?
        .0
        .main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|e| CoreError::CloneFailed(e.to_string()))?;

    Ok(ClonedRepo {
        path: target,
        _guard: temp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_url() {
        let p = parse_github_url("https://github.com/CBaileyDev/ProjectPacker").unwrap();
        assert_eq!(p.owner, "CBaileyDev");
        assert_eq!(p.repo, "ProjectPacker");
        assert_eq!(
            p.https_url,
            "https://github.com/CBaileyDev/ProjectPacker.git"
        );
    }

    #[test]
    fn parses_https_url_with_dot_git() {
        let p = parse_github_url("https://github.com/foo/bar.git").unwrap();
        assert_eq!(p.repo, "bar");
    }

    #[test]
    fn parses_git_at_form() {
        let p = parse_github_url("git@github.com:foo/bar.git").unwrap();
        assert_eq!(p.owner, "foo");
        assert_eq!(p.repo, "bar");
    }

    #[test]
    fn rejects_non_github_url() {
        let err = parse_github_url("https://gitlab.com/foo/bar").unwrap_err();
        assert!(matches!(err, CoreError::InvalidTarget(_)));
    }

    #[test]
    fn rejects_missing_repo() {
        let err = parse_github_url("https://github.com/owner-only").unwrap_err();
        assert!(matches!(err, CoreError::InvalidTarget(_)));
    }
}

use std::path::PathBuf;

fn isolated_project_slug(workspace: &str) -> Option<&str> {
    let (slug, tail) = workspace.split_once("-isolated-model-")?;
    let (pid, suffix) = tail.split_once('-')?;
    if slug.is_empty()
        || pid.is_empty()
        || !pid.chars().all(|c| c.is_ascii_digit())
        || suffix.is_empty()
        || !suffix.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(slug)
}

fn drive_prefix(value: &str) -> Option<&str> {
    value
        .as_bytes()
        .get(1)
        .filter(|&&b| b == b':')
        .map(|_| &value[..2])
}

fn resolve_slug(slug: &str, drive: Option<&str>) -> String {
    let owned_drive = std::env::var("SystemDrive").ok();
    let drive = drive.or(owned_drive.as_deref());
    if let Some(drive) = drive {
        let dev = PathBuf::from(format!(r"{drive}\dev"));
        let underscored = slug.replace('-', "_");
        for candidate in [slug, underscored.as_str()] {
            if dev.join(candidate).is_dir() {
                return candidate.to_owned();
            }
        }
    }
    slug.to_owned()
}

pub fn from_cwd(cwd: &str) -> String {
    let normalized = cwd.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    if let Some(i) = parts.iter().position(|p| p.eq_ignore_ascii_case("dev")) {
        if let Some(name) = parts.get(i + 1) {
            return (*name).into();
        }
    }
    let workspace = parts.last().copied().unwrap_or_default();
    let is_agentdock_tmp = parts.windows(2).any(|pair| {
        pair[0].eq_ignore_ascii_case(".agentdock") && pair[1].eq_ignore_ascii_case("tmp")
    });
    if is_agentdock_tmp {
        if let Some(slug) = isolated_project_slug(workspace) {
            return resolve_slug(slug, drive_prefix(cwd));
        }
    }
    if workspace.is_empty() {
        "unknown workspace".into()
    } else {
        workspace.to_owned()
    }
}

pub fn from_label(project: &str) -> String {
    isolated_project_slug(project)
        .map(|slug| resolve_slug(slug, None))
        .unwrap_or_else(|| project.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agentdock_isolated_model_workspace_collapses_to_project_slug() {
        assert_eq!(
            from_cwd(r"Z:\Users\fixture\.agentdock\tmp\fixture-app-isolated-model-38604-wE5Zfz"),
            "fixture-app"
        );
    }

    #[test]
    fn isolated_project_label_collapses_without_cwd() {
        assert_eq!(
            from_label("fixture-app-isolated-model-38604-wE5Zfz"),
            "fixture-app"
        );
    }

    #[test]
    fn normal_dev_workspace_keeps_real_project_folder() {
        assert_eq!(from_cwd(r"C:\dev\fixture_app\src"), "fixture_app");
    }

    #[test]
    fn unrelated_workspace_is_not_rewritten() {
        assert_eq!(
            from_cwd(r"Z:\Users\fixture\.agentdock\tmp\ordinary-workspace"),
            "ordinary-workspace"
        );
    }
}

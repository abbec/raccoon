use serde_json::Value;

use std::fmt;

pub fn dispatch<S: AsRef<str>>(kind: S, data: Value) -> Option<String> {
    match kind.as_ref() {
        "push" => Some(handle_push(data).to_string()),
        "tag_push" => Some(handle_tag_push(data).to_string()),
        "issue" => Some(handle_issue(data).to_string()),
        _ => None, // unknown event
    }
}

#[derive(Deserialize)]
struct PushEvent {
    user_name: String,
    total_commits_count: u32,
    repository: Repository,
}

#[derive(Deserialize)]
struct TagPushEvent {
    user_name: String,
    before: String,
    #[serde(rename = "ref")]
    tag_ref: String,
    repository: Repository,
}

#[derive(Deserialize)]
struct IssueEvent {
    user: User,
    #[serde(rename = "object_attributes")]
    issue: Issue,
    repository: Repository,
}

#[derive(Deserialize)]
struct User {
    name: String,
}

#[derive(Deserialize)]
struct Issue {
    title: String,
    url: String,
    action: String,
}

#[derive(Deserialize)]
struct Repository {
    name: String,
    homepage: String,
}

impl fmt::Display for PushEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "📌 {} pushed {} commits to {}",
            self.user_name, self.total_commits_count, self.repository
        )
    }
}

impl fmt::Display for TagPushEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let tag_name = self.tag_ref.rsplit('/').nth(0).unwrap_or("<invalid>");
        let action_text = if self.before == "0000000000000000000000000000000000000000" {
            "pushed"
        } else {
            "deleted"
        };

        write!(
            f,
            "🔖 {} {} tag \"{}\" to {}",
            self.user_name, action_text, tag_name, self.repository,
        )
    }
}

impl fmt::Display for IssueEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "🐛 {} {} on {}",
            self.user, self.issue, self.repository
        )
    }
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}ed issue \"{}\" ({})",
            self.action, self.title, self.url
        )
    }
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.homepage)
    }
}

fn handle_push(data: Value) -> PushEvent {
    serde_json::from_value(data).unwrap()
}

fn handle_tag_push(data: Value) -> TagPushEvent {
    serde_json::from_value(data).unwrap()
}

fn handle_issue(data: Value) -> IssueEvent {
    serde_json::from_value(data).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn push() {
        let tp = "push";
        let d = serde_json::from_reader(File::open("test/push.json").expect("find file")).unwrap();

        let s = dispatch(tp, d);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("pushed"));
        assert!(s.contains("commits to"));
    }

    #[test]
    fn push_tag() {
        let tp = "tag_push";
        let d =
            serde_json::from_reader(File::open("test/push_tag.json").expect("find file")).unwrap();

        let s = dispatch(tp, d);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("pushed tag \"v1.0.0\""));
    }

    #[test]
    fn issue() {
        let tp = "issue";
        let d = serde_json::from_reader(File::open("test/issue.json").expect("find file")).unwrap();

        let s = dispatch(tp, d);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("opened issue"));
    }
}

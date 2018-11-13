use serde_json::Value;

use std::fmt;

pub fn dispatch<S: AsRef<str>>(kind: S, data: Value) -> Option<String> {
    match kind.as_ref() {
        "push" => Some(handle_push(data).to_string()),
        "tag_push" => Some(handle_tag_push(data).to_string()),
        "issue" => Some(handle_issue(data).to_string()),
        "note" => Some(handle_comment(data).to_string()),
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
struct CommentEvent {
    user: User,
    repository: Repository,
    #[serde(rename = "object_attributes")]
    comment: Comment,
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

#[derive(Deserialize)]
struct Comment {
    noteable_type: String,
    url: String,
    note: String,
}

impl fmt::Display for PushEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "🌋 {} pushed {} commits to {}",
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

impl fmt::Display for CommentEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "💬 {} {}", self.user, self.comment)
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

impl fmt::Display for Comment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut msg = self.note.clone();

        // only show the first 40 chars
        const NCHARS: usize = 40;
        if self.note.len() > NCHARS {
            msg.truncate(NCHARS);
            msg = msg.trim_end().to_owned();
            msg.push_str("...");
        }
        write!(
            f,
            "commented on {} {}: {}",
            self.noteable_type.to_lowercase(),
            self.url,
            msg,
        )
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

fn handle_comment(data: Value) -> CommentEvent {
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

    #[test]
    fn commit_comment() {
        let tp = "note";
        let d = serde_json::from_reader(File::open("test/comment_commit.json").expect("find file"))
            .unwrap();

        let s = dispatch(tp, d);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("commented on"));
        assert!(s.contains("commit"));
    }

    #[test]
    fn mr_comment() {
        let tp = "note";
        let d = serde_json::from_reader(File::open("test/comment_mr.json").expect("find file"))
            .unwrap();

        let s = dispatch(tp, d);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("commented on"));
        assert!(s.contains("mergerequest"));
    }

    #[test]
    fn issue_comment() {
        let tp = "note";
        let d = serde_json::from_reader(File::open("test/comment_issue.json").expect("find file"))
            .unwrap();

        let s = dispatch(tp, d);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("commented on"));
        assert!(s.contains("issue"));
    }

    #[test]
    fn snippet_comment() {
        let tp = "note";
        let d =
            serde_json::from_reader(File::open("test/comment_snippet.json").expect("find file"))
                .unwrap();

        let s = dispatch(tp, d);
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("commented on"));
        assert!(s.contains("snippet"));
        assert!(s.ends_with("..."));
    }
}

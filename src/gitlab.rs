use serde_json::{error::Error as SerdeError, Value};

use std::fmt;

pub fn dispatch<S: AsRef<str>>(kind: S, data: Value, logger: &slog::Logger) -> Option<String> {
    match kind.as_ref() {
        "push" => {
            let res: Result<PushEvent, SerdeError> = serde_json::from_value(data);
            to_string(res, &logger)
        }
        "tag_push" => {
            let res: Result<TagPushEvent, SerdeError> = serde_json::from_value(data);
            to_string(res, &logger)
        }
        "issue" => {
            let res: Result<IssueEvent, SerdeError> = serde_json::from_value(data);
            to_string(res, &logger)
        }
        "note" => {
            let res: Result<CommentEvent, SerdeError> = serde_json::from_value(data);
            to_string(res, &logger)
        }
        "merge_request" => {
            let res: Result<MergeRequestEvent, SerdeError> = serde_json::from_value(data);
            to_string(res, &logger)
        }
        "wiki_page" => {
            let res: Result<WikiEvent, SerdeError> = serde_json::from_value(data);
            to_string(res, &logger)
        }
        "pipeline" => {
            let res: Result<PipelineEvent, SerdeError> = serde_json::from_value(data);
            to_string(res, &logger)
        }
        "build" => {
            let res: Result<BuildEvent, SerdeError> = serde_json::from_value(data);
            to_string(res, &logger)
        }
        _ => {
            warn!(logger, "unknown event type");
            None
        }
    }
}

fn to_string<T: fmt::Display>(res: Result<T, SerdeError>, logger: &slog::Logger) -> Option<String> {
    match res {
        Ok(pe) => Some(pe.to_string()),
        Err(e) => {
            error!(logger, "{}", e);
            None
        }
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
    #[serde(rename = "object_attributes")]
    comment: Comment,
}

#[derive(Deserialize)]
struct MergeRequestEvent {
    user: User,
    #[serde(rename = "object_attributes")]
    merge_request: MergeRequest,
    repository: Repository,
}

#[derive(Deserialize)]
struct WikiEvent {
    user: User,
    #[serde(rename = "object_attributes")]
    wiki_edit: WikiEditEvent,
}

#[derive(Deserialize)]
struct PipelineEvent {
    commit: Commit<String>,
    #[serde(rename = "object_attributes")]
    pipeline: Pipeline,
    project: Project,
}

#[derive(Deserialize)]
struct BuildEvent {
    commit: Commit<u32>,
    build_name: String,
    build_stage: String,
    build_status: String,
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

#[derive(Deserialize)]
struct Comment {
    noteable_type: String,
    url: String,
    note: String,
}

#[derive(Deserialize)]
struct MergeRequest {
    title: String,
    action: String,
    url: String,
}

#[derive(Deserialize)]
struct WikiEditEvent {
    title: String,
    action: String,
    url: String,
}

#[derive(Deserialize)]
struct Commit<T> {
    id: T,
    #[serde(default)]
    sha: String,
    message: String,
    #[serde(default)]
    url: String,
}

#[derive(Deserialize)]
struct Pipeline {
    status: String,
    #[serde(default)]
    duration: usize,
}

#[derive(Deserialize)]
struct Project {
    name: String,
    web_url: String,
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

impl fmt::Display for MergeRequestEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "🚓 {} {} on {}",
            self.user, self.merge_request, self.repository
        )
    }
}

impl fmt::Display for WikiEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "📰 {} {}", self.user, self.wiki_edit)
    }
}

impl fmt::Display for CommentEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "💬 {} {}", self.user, self.comment)
    }
}

impl fmt::Display for PipelineEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "👷 {} on {} for {}",
            self.pipeline, self.commit, self.project
        )
    }
}

impl fmt::Display for BuildEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "🚛 Build {} ({}) {} on {} for {}",
            self.build_name, self.build_stage, self.build_status, self.commit, self.repository
        )
    }
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl fmt::Display for Commit<String> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let first_line = self.message.split('\n').nth(0).unwrap_or("<invalid>");
        let mut shortid = self.id.clone();
        shortid.truncate(7);
        write!(f, "{}: {} ({})", shortid, first_line, self.url)
    }
}

impl fmt::Display for Commit<u32> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let first_line = self.message.split('\n').nth(0).unwrap_or("<invalid>");
        let mut shortid = self.sha.clone();
        shortid.truncate(7);
        write!(f, "{}: {}", shortid, first_line)
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

impl fmt::Display for MergeRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}ed merge request \"{}\" ({})",
            self.action, self.title, self.url
        )
    }
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.homepage)
    }
}

impl fmt::Display for Project {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.web_url)
    }
}

impl fmt::Display for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let duration = if self.duration > 0 {
            format!(" in {} seconds", self.duration)
        } else {
            String::new()
        };
        write!(f, "Pipeline {}{}", self.status, duration)
    }
}

impl fmt::Display for WikiEditEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ending = if self.action.ends_with('e') {
            "d"
        } else {
            "ed"
        };
        write!(
            f,
            "{}{} wiki page \"{}\" ({})",
            self.action, ending, self.title, self.url
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn push() {
        let tp = "push";
        let d = serde_json::from_reader(File::open("test/push.json").expect("find file")).unwrap();

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
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

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("pushed tag \"v1.0.0\""));
    }

    #[test]
    fn issue() {
        let tp = "issue";
        let d = serde_json::from_reader(File::open("test/issue.json").expect("find file")).unwrap();

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("opened issue"));
    }

    #[test]
    fn commit_comment() {
        let tp = "note";
        let d = serde_json::from_reader(File::open("test/comment_commit.json").expect("find file"))
            .unwrap();

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
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

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
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

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
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

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
        assert!(s.is_some());
        let s = s.unwrap();
        assert!(s.contains("commented on"));
        assert!(s.contains("snippet"));
        assert!(s.ends_with("supposed..."));
    }

    #[test]
    fn merge_request() {
        let tp = "merge_request";
        let d = serde_json::from_reader(File::open("test/merge_request.json").expect("find file"))
            .unwrap();

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
        assert!(s.is_some());
        let s = s.unwrap();

        assert!(s.contains("opened merge request"));
    }

    #[test]
    fn wiki_page() {
        let tp = "wiki_page";
        let d = serde_json::from_reader(File::open("test/wiki.json").expect("find file")).unwrap();

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
        assert!(s.is_some());
        let s = s.unwrap();

        assert!(s.contains("created wiki page"));
    }

    #[test]
    fn pipeline() {
        let tp = "pipeline";
        let d =
            serde_json::from_reader(File::open("test/pipeline.json").expect("find file")).unwrap();

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
        assert!(s.is_some());
        let s = s.unwrap();

        assert!(s.contains("Pipeline success"));
    }

    #[test]
    fn build() {
        let tp = "build";
        let d = serde_json::from_reader(File::open("test/build.json").expect("find file")).unwrap();

        let s = dispatch(tp, d, slog::Logger::root(slog::Discard, o!()));
        assert!(s.is_some());
        let s = s.unwrap();

        assert!(s.contains("Build"));
        assert!(s.contains("created"));
    }
}

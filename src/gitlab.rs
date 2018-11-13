use serde_json::Value;

use std::fmt;

pub fn dispatch<S: AsRef<str>>(kind: S, data: Value) -> Option<String> {
    match kind.as_ref() {
        "push" => {
            // TODO: make typed!
            Some(format!("{}", handle_push(data)))
        }

        _ => None, // unknown event
    }
}

#[derive(Deserialize)]
pub struct PushEvent {
    user_name: String,
    total_commits_count: u32,
    repository: Repository,
}

#[derive(Deserialize)]
pub struct Repository {
    name: String,
    homepage: String,
}

impl fmt::Display for PushEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "📌 {} pushed {} commits to {} ({})",
            self.user_name,
            self.total_commits_count,
            self.repository.name,
            self.repository.homepage
        )
    }
}

pub fn handle_push<'a>(data: Value) -> PushEvent {
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
}

#[macro_use]
extern crate serde_derive;

use gotham::router::builder::*;
use gotham::router::Router;
use gotham::state::{FromState, State};

use gotham::handler::{HandlerFuture, IntoHandlerError};
use gotham::helpers::http::response::create_empty_response;
use hyper::{Body, StatusCode};

use futures::{future::Future, stream::Stream};

mod gitlab;

fn router() -> Router {
    build_simple_router(|route| {
        route.post("/gitlab").to(handle_gitlab);
    })
}

fn handle_gitlab(mut state: State) -> Box<HandlerFuture> {
    let f = Body::take_from(&mut state).concat2().then(|b| match b {
        Ok(vb) => {
            match serde_json::from_slice(&vb) {
                Ok(json) => {
                    // determine kind and format message
                    let json: serde_json::Value = json;
                    let msg = gitlab::dispatch(
                        json["object_kind"].as_str().unwrap_or("bogus").to_owned(),
                        json,
                    );

                    // send message to irc
                    // TODO: maybe we should handle invalid cases here?
                    if let Some(m) = msg {
                        println!("{}", m);
                    }
                }
                Err(e) => return Err((state, e.into_handler_error())),
            }

            // return value is only used to signal that we
            // received the thing, so just send OK in case
            // we got down here 🦆
            let resp = create_empty_response(&state, StatusCode::OK);
            Ok((state, resp))
        }
        Err(e) => Err((state, e.into_handler_error())),
    });

    Box::new(f)
}

pub fn main() {
    let addr = "127.0.0.1:7878";
    println!("Listening for requests at http://{}", addr);
    gotham::start(addr, router())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gotham::test::TestServer;
    use hyper::StatusCode;
    use mime;

    #[test]
    fn gitlab_push() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/push.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_push_tag() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/push_tag.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_issue() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/issue.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_commit_comment() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/comment_commit.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_mr_comment() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/comment_mr.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_issue_comment() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/comment_issue.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_snippet_comment() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/comment_snippet.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_merge_request() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/merge_request.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_wiki() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/wiki.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_pipeline() {
        let test_server = TestServer::new(router()).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/pipeline.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}

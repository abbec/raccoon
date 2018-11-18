#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate slog;
#[macro_use]
extern crate gotham_derive;

use gotham::middleware::state::StateMiddleware;
use gotham::pipeline::{single::single_pipeline, single_middleware};
use gotham::router::builder::*;
use gotham::router::Router;
use gotham::state::{FromState, State};

use gotham::handler::{HandlerFuture, IntoHandlerError};
use gotham::helpers::http::response::create_empty_response;
use hyper::{Body, StatusCode};

use futures::{future::Future, stream::Stream};

use std::sync::Arc;

use slog::Drain;

mod gitlab;

#[derive(Clone, StateData)]
struct AppState {
    logger: Arc<slog::Logger>,
}

fn router(logger: slog::Logger) -> Router {
    let logger = AppState {
        logger: Arc::new(logger),
    };

    let middleware = StateMiddleware::new(logger);

    // create a middleware pipeline from our middleware
    let pipeline = single_middleware(middleware);

    // construct a basic chain from our pipeline
    let (chain, pipelines) = single_pipeline(pipeline);

    // build a router with the chain & pipeline
    build_router(chain, pipelines, |route| {
        route.post("/gitlab").to(handle_gitlab);
    })
}

fn handle_gitlab(mut state: State) -> Box<HandlerFuture> {
    let f = Body::take_from(&mut state).concat2().then(|b| match b {
        Ok(vb) => {
            match serde_json::from_slice(&vb) {
                Ok(json) => {
                    let app_state = AppState::borrow_from(&state);
                    let log = app_state.logger.new(o!());

                    // determine kind and format message
                    let json: serde_json::Value = json;
                    let object_kind = json["object_kind"]
                        .as_str()
                        .unwrap_or("no object kind")
                        .to_owned();
                    let msg = gitlab::dispatch(
                        &object_kind,
                        json,
                        log.new(o!("object_kind" => object_kind.clone())),
                    );

                    // send message to irc
                    if let Some(m) = msg {
                        debug!(log, "{}", m);
                    } else {
                        let resp = create_empty_response(&state, StatusCode::BAD_REQUEST);
                        return Ok((state, resp));
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
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    let log = slog::Logger::root(drain, o!());

    let addr = "127.0.0.1:7878";
    info!(log, "Listening for requests at http://{}", addr);
    gotham::start(addr, router(log))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gotham::test::TestServer;
    use hyper::StatusCode;
    use mime;

    #[test]
    fn gitlab_push() {
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
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

    #[test]
    fn gitlab_build() {
        let test_server = TestServer::new(router(slog::Logger::root(slog::Discard, o!()))).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/build.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}

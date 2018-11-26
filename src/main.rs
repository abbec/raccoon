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
use hyper::{Body, HeaderMap, StatusCode};

use futures::{future::Future, stream::Stream};

use std::sync::{Arc, RwLock};

use slog::Drain;

mod gitlab;
mod irc;

#[derive(Clone, StateData)]
struct AppState {
    logger: Arc<slog::Logger>,
    cfg: Arc<RwLock<config::Config>>,
}

fn router(logger: slog::Logger, cfg: config::Config) -> Router {
    let state = AppState {
        logger: Arc::new(logger),
        cfg: Arc::new(RwLock::new(cfg)),
    };

    let middleware = StateMiddleware::new(state);

    // create a middleware pipeline from our middleware
    let pipeline = single_middleware(middleware);

    // construct a basic chain from our pipeline
    let (chain, pipelines) = single_pipeline(pipeline);

    // build a router with the chain & pipeline
    build_router(chain, pipelines, |route| {
        route.post("/gitlab").to(handle_gitlab);
    })
}

fn compare_gitlab_token(headers: &HeaderMap, app_state: &AppState) -> Result<(), String> {
    match headers.get("X-Gitlab-Token") {
        Some(gl_token) => {
            let token: String = {
                let cfg = app_state.cfg.read().unwrap();
                cfg.get("gitlab_token")
                    .map_err(|e| format!("no gitlab_token in cfg: {}", e))?
            };

            if &token == gl_token {
                Ok(())
            } else {
                Err("mismatching gitlab token".to_owned())
            }
        }
        None => Err("no gitlab token in headers".to_owned()),
    }
}

fn handle_gitlab(mut state: State) -> Box<HandlerFuture> {
    let f = Body::take_from(&mut state).concat2().then(|b| match b {
        Ok(vb) => {
            let headers = HeaderMap::borrow_from(&state);
            match serde_json::from_slice(&vb) {
                Ok(json) => {
                    let app_state = AppState::borrow_from(&state);
                    let log = app_state.logger.new(o!());

                    // is this request something we want?
                    if let Err(e) = compare_gitlab_token(headers, app_state) {
                        error!(log, "Failed to validate Gitlab token: {}", e);
                        let resp = create_empty_response(&state, StatusCode::BAD_REQUEST);
                        return Ok((state, resp));
                    }

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

pub fn main() -> Result<(), String> {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    let log = slog::Logger::root(drain, o!());

    info!(log, "reading raccoon config file");
    let mut cfg = config::Config::default();
    match xdg::BaseDirectories::with_prefix("raccoon") {
        Ok(xdg_dirs) => {
            if let Some(f) = xdg_dirs.find_config_file("raccoon.toml") {
                info!(log, "using config file from {}", f.display());
                cfg.merge(config::File::with_name(
                    f.to_str().unwrap_or("invalid-string"),
                ))
                .map_err(|e| format!("failed to parse cfg at {}: {}", f.display(), e))?;
            }
        }
        Err(e) => warn!(log, "failed to get XDG directories: {}", e),
    };

    cfg.merge(config::File::with_name("./raccoon"))
        .map_err(|e| {
            error!(log, "failed to read config: {}", e);
            e.to_string()
        })?;

    cfg.merge(config::Environment::with_prefix("RACCOON"))
        .map_err(|e| {
            error!(log, "failed to read environment settings: {}", e);
            e.to_string()
        })?;

    irc::init(&cfg)?;

    let addr = "127.0.0.1:7878";
    info!(log, "Listening for requests at http://{}", addr);
    Ok(gotham::start(addr, router(log, cfg)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gotham::test::TestServer;
    use hyper::{header::HeaderValue, StatusCode};
    use mime;

    macro_rules! test_settings {
        () => {{
            let mut cfg = config::Config::default();
            cfg.set("gitlab_token", "TEST_TOKEN").unwrap();
            cfg
        }};
    }

    #[test]
    fn gitlab_invalid_token() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/push.json"),
                mime::APPLICATION_JSON,
            )
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn gitlab_push() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/push.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_push_tag() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/push_tag.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_issue() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/issue.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_commit_comment() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/comment_commit.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_mr_comment() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/comment_mr.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_issue_comment() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/comment_issue.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_snippet_comment() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/comment_snippet.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_merge_request() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/merge_request.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_wiki() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/wiki.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_pipeline() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/pipeline.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_build() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
        ))
        .unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/gitlab/",
                include_str!("../test/build.json"),
                mime::APPLICATION_JSON,
            )
            .with_header("X-Gitlab-Token", HeaderValue::from_static("TEST_TOKEN"))
            .perform()
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}

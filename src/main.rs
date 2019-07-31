#![deny(warnings)]

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
use gotham::helpers::http::response::{create_empty_response, create_response};
use hyper::{Body, HeaderMap, StatusCode};

use futures::{future::Future, stream::Stream};

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use serde_json::json;

use slog::Drain;

use structopt::StructOpt;

mod gitlab;
mod irc;

#[derive(Clone, StateData)]
struct AppState {
    logger: Arc<slog::Logger>,
    cfg: Arc<RwLock<config::Config>>,
    irc: Arc<Mutex<Box<irc::IrcWriter + Send>>>,
}

fn router(logger: slog::Logger, cfg: config::Config, irc: Box<irc::IrcWriter + Send>) -> Router {
    let state = AppState {
        logger: Arc::new(logger),
        cfg: Arc::new(RwLock::new(cfg)),
        irc: Arc::new(Mutex::new(irc)),
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
            let token: String = app_state
                .cfg
                .read()
                .map_err(|e| format!("failed to lock application config for reading: {}", e))
                .and_then(|cfg| {
                    cfg.get("gitlab.token")
                        .map_err(|e| format!("no gitlab.token in cfg: {}", e))
                })?;

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
                        &log.new(o!("object_kind" => object_kind.clone())),
                    );

                    // send message to irc
                    match msg {
                        Ok(m) => {
                            debug!(log, "{}", m);
                            if let Err(e) = app_state
                                .irc
                                .lock()
                                .map_err(|_| String::from("failed to obtain irc writer lock"))
                                .and_then(|mut i| i.write(&m))
                            {
                                error!(log, "failed to post message to IRC: {}", e);
                            }
                        }
                        Err(e) => {
                            let resp = create_response(
                                &state,
                                StatusCode::BAD_REQUEST,
                                mime::APPLICATION_JSON,
                                json!({
                                    "code": 400,
                                    "error": {
                                        "message": format!("Failed to parse Gitlab payload: {}", e)
                                    }
                                })
                                .to_string(),
                            );
                            return Ok((state, resp));
                        }
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

#[derive(StructOpt, Debug)]
/// Raccoon is a service that accepts Gitlab HTTP hooks as described at
/// https://docs.gitlab.com/ee/user/project/integrations/webhooks.html
/// and sends the resulting formatted text to IRC.
struct Opt {
    #[structopt(parse(from_os_str), short = "c", long = "config")]
    /// Config file to use. This overrides the standard config
    /// file resolution. See man page for config file format and
    /// resolution order if this parameter is not specified.
    config: Option<PathBuf>,

    #[structopt(short = "p", long = "port")]
    /// Port to bind the service to, default is 7878.
    /// Can also be set in the settings file with the setting `service.port`.
    port: Option<u16>,

    #[structopt(short = "b", long = "bind")]
    /// Address to bind the service to, default is 127.0.0.1.
    /// Can also be set in the settings file with the setting `service.bind`.
    bind: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ServiceConfig {
    bind: String,
    port: u16,
}

pub fn main() -> Result<(), String> {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    let log = slog::Logger::root(drain, o!());

    let opt = Opt::from_args();

    let mut cfg = config::Config::default();
    match opt.config {
        Some(c) => {
            info!(
                log,
                "reading raccoon config file {} as specified on the command line",
                c.display()
            );

            cfg.merge(config::File::with_name(
                c.to_str().unwrap_or("<invalid-string>"),
            ))
            .map_err(|e| {
                error!(log, "failed to read config: {}", e);
                e.to_string()
            })?;
        }
        None => {
            info!(log, "reading raccoon config file");
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

            if Path::new("./raccoon").exists() {
                info!(log, "using config file in current directory");
                cfg.merge(config::File::with_name("./raccoon"))
                    .map_err(|e| {
                        error!(log, "failed to read config: {}", e);
                        e.to_string()
                    })?;
            }
        }
    }

    cfg.merge(config::Environment::with_prefix("RACCOON"))
        .map_err(|e| {
            error!(log, "failed to read environment settings: {}", e);
            e.to_string()
        })?;

    info!(log, "connecting to IRC");
    let writer = irc::RealIrcWriter::new(irc::init(&cfg, &log)?);

    cfg.set_default("service.bind", "127.0.0.1".to_owned())
        .map_err(|e| {
            error!(
                log,
                "failed to set default value for service.bind setting: {}", e
            );
            e.to_string()
        })?;
    cfg.set_default("service.port", 7878).map_err(|e| {
        error!(
            log,
            "failed to set default value for service.bind setting: {}", e
        );
        e.to_string()
    })?;

    let service_config: ServiceConfig = cfg.get("service").map_err(|e| {
        error!(log, "failed to parse service settings: {}", e);
        e.to_string()
    })?;

    let addr = format!(
        "{}:{}",
        opt.bind.unwrap_or(service_config.bind),
        opt.port.unwrap_or(service_config.port)
    );

    info!(log, "Listening for requests at http://{}", addr);
    gotham::start(addr, router(log, cfg, Box::new(writer)));

    Ok(())
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
            cfg.set("gitlab.token", "TEST_TOKEN").unwrap();
            cfg
        }};
    }

    #[derive(Clone)]
    pub struct FakeIrcWriter {
        pub buffer: Arc<RwLock<String>>,
    }

    impl FakeIrcWriter {
        pub fn new() -> Self {
            FakeIrcWriter {
                buffer: Arc::new(RwLock::new(String::new())),
            }
        }

        pub fn contains(&self, sub: &str) -> bool {
            let s = self.buffer.read().unwrap();
            s.contains(sub)
        }
    }

    impl irc::IrcWriter for FakeIrcWriter {
        fn write(&mut self, message: &str) -> Result<(), String> {
            let mut b = self.buffer.write().unwrap();
            b.push_str(message);
            Ok(())
        }
    }

    #[test]
    fn gitlab_invalid_token() {
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(FakeIrcWriter::new()),
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
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("pushed"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_push_tag() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("pushed tag"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_issue() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("opened issue"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_commit_comment() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("commented on"));
        assert!(irc.contains("commit"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_mr_comment() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("commented on"));
        assert!(irc.contains("mergerequest"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_issue_comment() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("commented on"));
        assert!(irc.contains("issue"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_snippet_comment() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("commented on"));
        assert!(irc.contains("snippet"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_merge_request() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("opened merge request"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_wiki() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("created wiki page"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_pipeline() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("Pipeline success"));
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn gitlab_build() {
        let irc = FakeIrcWriter::new();
        let test_server = TestServer::new(router(
            slog::Logger::root(slog::Discard, o!()),
            test_settings!(),
            Box::new(irc.clone()),
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

        assert!(irc.contains("Build"));
        assert!(irc.contains("created"));
        assert_eq!(response.status(), StatusCode::OK);
    }
}

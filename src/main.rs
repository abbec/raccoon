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
            let c: serde_json::Value = serde_json::from_slice(&vb).unwrap();

            // determine kind and format message
            let msg = gitlab::dispatch(c["object_kind"].as_str().unwrap_or("bogus").to_owned(), c);

            // send message to irc
            if let Some(m) = msg {
                println!("{}", m);
            }

            // return value is only used to signal that we
            // received the thing
            let resp = create_empty_response(&state, StatusCode::OK);
            Ok((state, resp))
        }
        Err(e) => Err((state, e.into_handler_error())),
    });

    Box::new(f)
}

/// Start a server and use a `Router` to dispatch requests
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
    fn gitlab() {
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
}

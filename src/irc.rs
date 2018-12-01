use irc::client::{self, ext::ClientExt};
use std::{collections::HashMap, sync::mpsc, thread};

pub use irc::client::Client;

#[derive(Deserialize, Debug)]
struct IrcConfig {
    nickname: String,
    nick_password: String,
    server: String,
    port: u16,
    channels: Vec<String>,
}

pub struct RealIrcWriter {
    client: client::IrcClient,
}

impl RealIrcWriter {
    pub fn new(client: client::IrcClient) -> Self {
        RealIrcWriter { client: client }
    }
}

pub trait IrcWriter {
    fn write(&mut self, message: &str) -> Result<(), String>;
}

impl IrcWriter for RealIrcWriter {
    fn write(&mut self, message: &str) -> Result<(), String> {
        self.client
            .send(message)
            .map_err(|e| format!("failed to send IRC message: {}", e))
    }
}

impl From<IrcConfig> for client::data::config::Config {
    fn from(cfg: IrcConfig) -> Self {
        let (chans, keys) = split_channel_keys(cfg.channels);
        client::data::config::Config {
            nickname: Some(cfg.nickname),
            nick_password: Some(cfg.nick_password),
            server: Some(cfg.server),
            port: Some(cfg.port),
            channels: Some(chans),
            use_ssl: Some(true),
            channel_keys: Some(keys),
            ..client::data::config::Config::default()
        }
    }
}

fn split_channel_keys(channels: Vec<String>) -> (Vec<String>, HashMap<String, String>) {
    (
        channels
            .iter()
            .map(|c| c.split(':').nth(0).unwrap().to_owned())
            .collect(),
        channels
            .iter()
            .filter_map(|c| {
                let mut parts = c.split(':');
                parts
                    .nth(0)
                    // the iterator does not rewind so need to use 0 again
                    .and_then(|chan| match parts.nth(0) {
                        Some(k) => Some((chan, k)),
                        None => None,
                    })
                    .and_then(|(chan, key)| Some((chan.to_owned(), key.to_owned())))
            })
            .collect(),
    )
}

pub fn init(config: &config::Config, logger: &slog::Logger) -> Result<client::IrcClient, String> {
    let (tx, rx) = mpsc::channel();
    let log = logger.new(o!());

    let parsed: IrcConfig = config
        .get("irc")
        .map_err(|e| format!("failed to parse irc config: {}", e))?;

    thread::spawn(move || -> Result<(), String> {
        let mut reactor = client::reactor::IrcReactor::new()
            .map_err(|e| format!("failed to create IRC reactor: {}", e))?;
        let client = reactor
            .prepare_client_and_connect(&parsed.into())
            .map_err(|e| format!("failed to connect IRC client: {}", e))?;
        tx.send(client.clone()).unwrap();

        client
            .identify()
            .map_err(|e| format!("failed to identify: {}", e))?;

        reactor.register_client_with_handler(client.clone(), |_, _| Ok(()));

        info!(log, "starting IRC event loop");
        reactor
            .run()
            .map_err(|e| format!("failed to start IRC event loop: {}", e))?;

        Ok(())
    });

    Ok(rx
        .recv()
        .map_err(|e| format!("failed to recieve irc client: {}", e))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_channel_keys() {
        let chans = vec![
            String::from("#testchannel:password"),
            String::from("#nopasschannel"),
            String::from("#another-channel"),
        ];

        let (channels, keys) = split_channel_keys(chans);

        assert_eq!(channels.len(), 3);
        assert_eq!(keys.len(), 1);

        assert!(keys.contains_key("#testchannel"));
        assert_eq!(keys["#testchannel"], "password");
    }
}

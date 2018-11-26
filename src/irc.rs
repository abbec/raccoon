use irc::client::{self, ext::ClientExt};
use std::collections::HashMap;

pub use irc::client::Client;

#[derive(Deserialize)]
struct IrcConfig {
    nickname: String,
    nick_password: String,
    server: String,
    port: u16,
    channels: Vec<String>,
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
                    .and_then(|chan| match parts.nth(1) {
                        Some(k) => Some((chan, k)),
                        None => None,
                    })
                    .and_then(|(chan, key)| Some((chan.to_owned(), key.to_owned())))
            })
            .collect(),
    )
}

pub fn init(config: &config::Config) -> Result<client::IrcClient, String> {
    let parsed: IrcConfig = config
        .get("irc")
        .map_err(|e| format!("failed to parse irc config: {}", e))?;

    let mut reactor = client::reactor::IrcReactor::new().unwrap();
    let client = reactor
        .prepare_client_and_connect(&parsed.into())
        .map_err(|e| format!("failed to connect to IRC: {}", e))?;
    client
        .identify()
        .map_err(|e| format!("failed to identify: {}", e))?;

    Ok(client)
}

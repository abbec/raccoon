[![Build Status](https://travis-ci.com/abbec/raccoon.svg?branch=master)](https://travis-ci.com/abbec/raccoon)

# 🦝 Raccoon

is an IRC notifier for Gitlab HTTP hooks.

# Configuration

Raccoon is configured with a [TOML](https://github.com/toml-lang/toml) file. The first thing that is
needed is setting up a Gitlab token. In the Gitlab UI, create a webhook with the events that you
like and set the "Secret Token" to something of your liking. In the raccoon config file, specify the
same token as

```toml
[gitlab]
token = "YOUR_SECRET_TOKEN"
```

Configuration for IRC is specified under the `irc` key

```toml
[irc]
nickname = "your_nick"
nick_password = "secret_stuff"
server = "irc.server.org"
port = 6697
channels = ["#channel1", "#channel_with_key:the_key"]
```
Currently, Raccoon only supports IRC servers with SSL enabled.

Config files are read from (in order)

- `$XDG_CONFIG_HOME/raccoon/raccoon.toml`
- `$XDG_CONFIG_DIRS/raccooon/raccoon.toml` (usually `/etc/xdg/raccoon/raccoon.toml`)
- `./raccoon.toml`

# Developing

- Install Rust: https://rustup.rs
- Build the code with `cargo build`
- Run tests with `cargo test`
- Check lints with `cargo clippy`
- Check format with `cargo fmt -- --check` or let rustfmt format the code with `cargo fmt`

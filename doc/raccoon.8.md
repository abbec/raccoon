% RACCOON(8) Raccoon User Manual | Version 0.1.0
% Albert Cervin <albert@acervin.com>
% December 2018

# NAME

**raccoon** - accepts Gitlab HTTP webhooks and sends to IRC

# SYNOPSIS

**raccoon**

# DESCRIPTION

Raccoon is a service that accepts Gitlab HTTP hooks as described at
https://docs.gitlab.com/ee/user/project/integrations/webhooks.html and sends the resulting
formatted text to IRC.

# OPTIONS

None yet

# CONFIGURATION

Raccoon searches for configuration files in the following order when starting up:

_$XDG_CONFIG_HOME/raccoon/raccoon.toml_, usually _~/.config/raccoon/raccoon.toml_

_$XDG_CONFIG_DIRS/raccoon/raccoon.toml_, usually _/etc/xdg/raccoon/raccoon.toml_

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

# HOMEPAGE

https://github.com/abbec/raccoon

Please report bugs in the Github issue tracker


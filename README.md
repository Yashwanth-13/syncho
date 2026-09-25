# syncho

[![Rust](https://img.shields.io/badge/Rust-f74c00?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)

A Rust CLI that synchronizes music playback between [Cider](https://cider.sh) and Spotify, letting multiple listeners on a LAN stay in sync as one host controls playback.

## How it works

- Everyone needs either [Cider](https://cider.sh) with an Apple music subscription or a Spotify subscription. 
- Host starts a session and shares 4-character numeric join code.
- Clients connect using that code and host's IP.
- The host controls playback, everyone will be in sync.

## Building

### Prerequisites

- [Rust](https://rustup.rs/) (1.88+)

### Compilation

```bash
git clone https://github.com/Yashwanth-13/syncho.git
cd syncho
cargo build --release
```

## Usage

Run the built binary and follow the interactive prompts to either host a session or join one with a code:

```bash
cargo run --release -- --help
```

From there:
- **Host:** starts a session, prints a join code, and begins broadcasting song changes as playback changes on their machine.
- **Join:** connects to a host's session using their code and mirrors playback locally through their player.

> Run with `--help` for options. Host can do `help` to see their options, even though they won't be needed much.

## Requirements

- This program needs API Tokens for the music player that you use to control playback. All the tokens are stored locally in a file called `.syncho` under the user's home directory. *(You need to enter this only for the first usage)*
    - Cider - Get it from the app. `Settings > Connections > Manage external applications access to Cider`
    - Spotify - Read the [Spotify WebAPI](https://developer.spotify.com/documentation/web-api) documentation. You will need: 
        - Refresh token
        - Client ID
        - Client Secret
        - Access Token

## Connectivity

- We only built this for LAN-connected systems. However, you can build a [Tailnet](https://tailscale.com/) with your friends to connect securely over the internet and use this program as if you are on the same network.

> There's a plan to work towards P2P connectivity over the internet thorugh libp2p but no promises!

## License

MIT. Share your changes with me, or don't! It's code, do whatever you want with it.
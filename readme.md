# ClipSynk

Real-time lightweight clipboard sync over your local network.

Copy on one device, paste on another — no cloud, no account, no config.

## Features

- **Zero config** — auto-discovers peers on the LAN via UDP broadcast
- **Cross-platform** — Linux (Wayland + X11), Windows, macOS
- **Lightweight** — single binary, minimal resource usage
- **Real-time** — clipboard changes sync instantly over TCP
- **Peer-to-peer** — no server, no cloud, data never leaves your network

## Install

*Before installing you should have rust+cargo installed*

From crates.io:

```sh
cargo install clipsynk
```

From GitHub:

```sh
git clone https://github.com/hereparvezali/clipsynk.git
cd clipsynk
cargo build --release
```

The binary will be at `target/release/clipsynk`.

## Usage

Run on each device connected to the same local network:

```sh
clipsynk
```

That's it. Devices discover each other automatically. Copy text on one device and it appears on all others.

## How it works

1. Each instance broadcasts its presence via UDP on the LAN
2. Peers discover each other and establish persistent TCP connections
3. Clipboard changes are detected, hashed, and sent as framed messages
4. Receiving devices update their clipboard if the content is new

## Platform support

| Platform | Clipboard backend |
|----------|-------------------|
| Linux (Wayland) | `wl-clipboard-rs` + `wayland-clipboard-listener` |
| Linux (X11) | `arboard` + `clipboard-master` |
| Windows | `clipboard-win` + `clipboard-master` |
| macOS | `arboard` + `clipboard-master` |


## License

MIT

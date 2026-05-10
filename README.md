# cargo-min-publish-age
This project provides a temporary solution for addressing supply-chain attacks in Rust ecosystem.
Namely, it provides a local [index.crates.io](https://index.crates.io) proxy
that marks all packages published recently (e.g. < 14 days ago) as yanked.

However, while developing this solution, I learned about an existing
[publicly hosted proxy by Menhera](https://www.menhera.org/crates-io-cooldown-proxy-mitigating-supply-chain-attacks/).
Users are recommended that solution until Rust [RFC 3923](https://github.com/rust-lang/rfcs/pull/3923)
is merged and [implemented](https://github.com/rust-lang/cargo/issues/15973).

## Usage

1. Install from source

```sh
cargo install --git https://github.com/dinvlad/cargo-min-publish-age
```

2. Install user-owned proxy service on localhost via SystemD (on Linux) or LaunchAgent (on macOS),
and configure Cargo to always go through it:

```sh
cargo min-publish-age install

# Optional: use a different port  (default: 2015) or age cutoff (default: 14 days)
cargo min-publish-age install --port 2024 --age "7 days"
```

3. Uninstall to stop the service and restore `~/.cargo/config.toml`:

```sh
cargo min-publish-age uninstall
```

### Notes

- This solution has been tested on Linux with SystemD, but yet to be tested on macOS
- I purposefully didn't publish it on Crates.io to avoid potential user confusion at officially named `min-publish-age` option,
  and that source provenance works better with a direct Git package URL anyway
- While it has worked for me, please use at your own risk :-)

# cargo-min-publish-age
This project provides a temporary solution for addressing supply-chain attacks in Rust ecosystem.
Namely, it provides a local [index.crates.io](https://index.crates.io) proxy
that marks all packages published recently (e.g. < 14 days ago) as yanked.

However, while developing this solution, I learned about an existing
[publicly hosted proxy by Menhera](https://www.menhera.org/crates-io-cooldown-proxy-mitigating-supply-chain-attacks/).
Users are recommended that solution until Rust [RFC 3923](https://github.com/rust-lang/rfcs/pull/3923)
is merged and [implemented](https://github.com/rust-lang/cargo/issues/15973).

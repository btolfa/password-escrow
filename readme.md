# Distributor

### Prerequisites

- (Rust) [rustup](https://www.rust-lang.org/tools/install)
- (Solana) [solana-cli](https://docs.solana.com/cli/install-solana-cli-tools) 1.18.1
- (Anchor) [anchor](https://www.anchor-lang.com/docs/installation) 0.29.0
- (Node) [node](https://github.com/nvm-sh/nvm) 18.18.0

### Build and run tests

```bash
anchor build
anchor test --provider.cluster localnet
```
# Veritasor Contracts

Soroban smart contracts for the Veritasor revenue attestation protocol on Stellar. Store revenue Merkle roots and metadata on-chain; full data remains off-chain.

## Contract: `attestation`

Stores one attestation per (business address, period). Each attestation is a Merkle root (32 bytes), timestamp, and version. Duplicate (business, period) submissions are rejected.

### Methods

| Method | Description |
|--------|-------------|
| `submit_attestation(business, period, merkle_root, timestamp, version)` | Store attestation. Panics if one already exists for this business and period. |
| `get_attestation(business, period)` | Returns `Option<(BytesN<32>, u64, u32)>`. |
| `verify_attestation(business, period, merkle_root)` | Returns `true` if an attestation exists and its root matches. |
| `init(admin)` | One-time setup of admin for anomaly feature. |
| `add_authorized_analytics(caller, analytics)` | Add an authorized analytics/oracle address (admin only). |
| `remove_authorized_analytics(caller, analytics)` | Remove an authorized analytics address (admin only). |
| `set_anomaly(updater, business, period, flags, score)` | Store anomaly flags and risk score (authorized updaters only; score 0–100). |
| `get_anomaly(business, period)` | Returns `Option<(u32, u32)>` (flags, score) for lenders. |

### Prerequisites

- Rust 1.75+
- Soroban CLI (optional, for deployment): [Stellar Soroban docs](https://developers.stellar.org/docs/build/smart-contracts)

### Setup

```bash
# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add wasm target for Soroban
rustup target add wasm32-unknown-unknown

# Build the contract
cd contracts/attestation
cargo build --target wasm32-unknown-unknown --release
```

The `.wasm` artifact will be in `target/wasm32-unknown-unknown/release/veritasor_attestation.wasm` (name may vary by crate name).

### Tests

```bash
cd contracts/attestation
cargo test
```

### Project structure

```
veritasor-contracts/
├── Cargo.toml              # Workspace root
├── docs/
│   └── attestation-anomaly-flags.md   # Anomaly flags and risk scores
└── contracts/
    └── attestation/
        ├── Cargo.toml
        └── src/
            ├── lib.rs         # Contract logic
            ├── test.rs        # Unit tests
            └── anomaly_test.rs  # Anomaly feature tests
```

### Deploying (Stellar / Soroban CLI)

With [Stellar CLI](https://developers.stellar.org/docs/tools/stellar-cli) and a configured network:

```bash
stellar contract deploy \
  --network testnet \
  --source <KEY> \
  target/wasm32-unknown-unknown/release/veritasor_attestation.wasm
```

### Merging to remote

This directory is its own git repository. To push to your remote:

```bash
git remote add origin <your-contracts-repo-url>
git push -u origin main
```

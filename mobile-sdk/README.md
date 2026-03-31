# COINjecture Mobile SDK

Lightweight, standalone SDK for COINjecture light clients on mobile and web platforms.

## Features

- **FlyClient Protocol**: Super-light verification with O(log n) proof size
- **MMR Proofs**: Merkle Mountain Range inclusion proofs for block verification
- **Transaction Proofs**: SPV-style transaction verification
- **Cross-Platform**: Compiles to WASM, iOS, and Android

## Size Budget

- Compiled WASM: < 200KB (gzipped: < 50KB)
- No async runtime dependencies
- Minimal std usage

## Building

### WASM (Web)

```bash
# Install wasm-pack
cargo install wasm-pack

# Build for web
wasm-pack build --target web --release --features wasm

# Build for bundlers (webpack, etc.)
wasm-pack build --target bundler --release --features wasm
```

### iOS

```bash
# Add target
rustup target add aarch64-apple-ios

# Build
cargo build --target aarch64-apple-ios --release --features ffi
```

### Android

```bash
# Add target
rustup target add aarch64-linux-android

# Build (requires Android NDK)
cargo build --target aarch64-linux-android --release --features ffi
```

## Usage

### JavaScript/TypeScript (WASM)

```typescript
import init, { WasmLightClient } from 'coinject-mobile-sdk';

// Initialize WASM
await init();

// Create light client with genesis hash
const client = new WasmLightClient(
  '0000000000000000000000000000000000000000000000000000000000000000'
);

// Get verified height
console.log('Height:', client.height);

// Verify MMR proof
const isValid = client.verify_mmr_proof(proofJson);

// Export/Import state
const state = client.export_json();
const restored = WasmLightClient.import_json(state);
```

### Swift (iOS)

```swift
import CoinjectMobile

// Create light client
let client = coinject_light_client_new(genesisHex.cString(using: .utf8))

// Get height
let height = coinject_light_client_height(client)

// Verify proof
let result = coinject_verify_mmr_proof(client, proofJson.cString(using: .utf8))

// Free when done
coinject_light_client_free(client)
```

### Kotlin (Android)

```kotlin
import com.coinject.mobile.*

// Create light client
val client = coinject_light_client_new(genesisHex)

// Get height
val height = coinject_light_client_height(client)

// Verify proof
val result = coinject_verify_mmr_proof(client, proofJson)

// Free when done
coinject_light_client_free(client)
```

## API Reference

### Core Types

- `Hash`: 32-byte cryptographic hash
- `BlockHeader`: Compact block header (92 bytes)
- `MMRProof`: Merkle Mountain Range inclusion proof
- `FlyClientProof`: Super-light chain verification proof
- `TxProof`: Transaction inclusion proof
- `LightClient`: Stateful light client verifier

### Verification Methods

| Method | Description | Proof Size |
|--------|-------------|-----------|
| `verify_flyclient()` | Verify entire chain state | O(log n) |
| `verify_block()` | Verify single block inclusion | O(log n) |
| `verify_transaction()` | Verify tx in verified block | O(log n) + O(log m) |

## Security

The SDK implements the FlyClient protocol from the paper:
> "FlyClient: Super-Light Clients for Cryptocurrencies"
> https://eprint.iacr.org/2019/226.pdf

Security parameter λ = 50 provides 2^-50 security against adversaries with <50% hash power.

## License

MIT OR Apache-2.0


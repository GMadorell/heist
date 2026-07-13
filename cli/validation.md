## Build

```bash
cargo build --manifest-path cli/Cargo.toml
```

## Lint

```bash
cargo clippy --manifest-path cli/Cargo.toml --all-targets -- -D warnings
```

```bash
cargo fmt --manifest-path cli/Cargo.toml -- --check
```

## Test

all:
```bash
cargo test --manifest-path cli/Cargo.toml
```

single:
```bash
cargo test --manifest-path cli/Cargo.toml -- --exact <test_name>
```

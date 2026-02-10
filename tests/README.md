# VirtualESC Tests

This directory contains Rust integration tests for the virtualesc project.

## Test Structure

```
tests/
├── tcp_server.rs         # TCP server integration tests
├── command_parsing.rs    # Command parsing unit tests (examples)
└── README.md            # This file

test/                     # Manual/shell-based tests
├── test_text.sh
├── test_qr.sh
├── test_raster.sh
├── test_combined.sh
└── run_all.sh
```

## Running Tests

### Rust Integration Tests

The integration tests in `tests/tcp_server.rs` are currently **ignored by default** because they require a running virtualesc server.

To run all tests (excluding ignored):
```bash
cargo test
```

To run ignored tests (requires virtualesc running on port 9100):
```bash
# Terminal 1: Start virtualesc
./target/release/virtualesc

# Terminal 2: Run ignored tests
cargo test -- --ignored
```

To run a specific test:
```bash
cargo test test_tcp_connection -- --ignored
```

To run all tests including ignored:
```bash
cargo test -- --include-ignored
```

### Manual Shell Tests

See `test/README.md` or run:
```bash
./test/run_all.sh
```

## Test Coverage

### Integration Tests (`tests/tcp_server.rs`)

- ✅ `test_tcp_connection` - Basic TCP connectivity
- ✅ `test_simple_text` - Text rendering
- ✅ `test_text_formatting` - Bold text
- ✅ `test_qr_code` - QR code generation
- ✅ `test_raster_graphics` - Raster image rendering
- ⚠️  `test_status_query` - Status responses (not yet implemented)
- ✅ `test_multiple_connections` - Concurrent connections

### Unit Tests (`tests/command_parsing.rs`)

These are **example tests** showing what should be tested once the code is refactored to be more testable. They don't currently run because they need access to internal functions.

To make them work, consider:
1. Adding `#[cfg(test)]` modules directly in `src/main.rs`
2. Refactoring `EscPosRenderer` into a separate module with public methods
3. Exposing command parsing functions for testing

## Future Improvements

1. **Refactor for testability**
   - Extract `EscPosRenderer` to `src/renderer.rs`
   - Make command parsing functions testable
   - Separate GUI from server logic

2. **Add more tests**
   - Test all ESC/POS commands
   - Test error handling
   - Test edge cases (partial commands, buffer overflow, etc.)

3. **Implement status responses**
   - Make `test_status_query` pass
   - Support DLE EOT, DLE ENQ commands
   - Enable receiptio compatibility

4. **Property-based testing**
   - Use `proptest` or `quickcheck` for fuzzing
   - Generate random ESC/POS sequences

## Writing New Tests

### Integration Test Example

```rust
#[tokio::test]
#[ignore]
async fn test_my_feature() {
    let mut stream = connect_to_server().await.unwrap();
    stream.write_all(b"\x1B\x40Hello").await.unwrap();
    // Assert expected behavior
}
```

### Unit Test Example (for src/main.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_parsing() {
        let mut renderer = EscPosRenderer::new(false);
        renderer.process_data(b"\x1B\x40").unwrap();
        // Assert expected state
    }
}
```

## CI/CD Integration

For automated testing in CI:

```yaml
# .github/workflows/test.yml
- name: Run tests
  run: cargo test

# Integration tests would need a mock server or refactoring
```

// Integration tests for TCP server functionality
// Note: These tests require the main code to be refactored to expose
// the TCP server separately from the GUI

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const TEST_PORT: u16 = 9100;

/// Helper to create a TCP connection to the virtualesc server
async fn connect_to_server() -> Result<TcpStream, std::io::Error> {
    TcpStream::connect(format!("127.0.0.1:{}", TEST_PORT)).await
}

/// Helper to send data and optionally wait for response
async fn send_escpos_data(data: &[u8]) -> Result<(), std::io::Error> {
    let mut stream = connect_to_server().await?;
    stream.write_all(data).await?;
    stream.flush().await?;

    // Give server time to process
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(())
}

#[tokio::test]
#[ignore] // Ignored by default - requires running server
async fn test_tcp_connection() {
    let result = connect_to_server().await;
    assert!(
        result.is_ok(),
        "Should connect to TCP server on port {}",
        TEST_PORT
    );
}

#[tokio::test]
#[ignore] // Ignored by default - requires running server
async fn test_simple_text() {
    let data = b"\x1B\x40Hello World\x0A"; // ESC @ (init) + text + LF
    let result = send_escpos_data(data).await;
    assert!(result.is_ok(), "Should successfully send text data");
}

#[tokio::test]
#[ignore] // Ignored by default - requires running server
async fn test_text_formatting() {
    // Test bold text
    let data = b"\x1B\x40\x1B\x45\x01Bold\x1B\x45\x00\x0A";
    let result = send_escpos_data(data).await;
    assert!(result.is_ok(), "Should successfully send bold text");
}

#[tokio::test]
#[ignore] // Ignored by default - requires running server
async fn test_qr_code() {
    // QR code command sequence
    let mut data = Vec::new();
    data.extend_from_slice(b"\x1B\x40"); // Init
    data.extend_from_slice(b"\x1D\x28\x6B\x04\x00\x31\x41\x32\x00"); // Set QR model
    data.extend_from_slice(b"\x1D\x28\x6B\x03\x00\x31\x43\x05"); // Set QR size
    data.extend_from_slice(b"\x1D\x28\x6B\x03\x00\x31\x45\x30"); // Set error correction
    data.extend_from_slice(b"\x1D\x28\x6B\x10\x00\x31\x50\x30https://test.com"); // Store data
    data.extend_from_slice(b"\x1D\x28\x6B\x03\x00\x31\x51\x30"); // Print QR

    let result = send_escpos_data(&data).await;
    assert!(result.is_ok(), "Should successfully send QR code");
}

#[tokio::test]
#[ignore] // Ignored by default - requires running server
async fn test_raster_graphics() {
    // Simple 8x8 checkerboard pattern
    let mut data = Vec::new();
    data.extend_from_slice(b"\x1B\x40"); // Init
    data.extend_from_slice(b"\x1B\x2A\x00\x08\x00"); // ESC * mode 0, 8 bytes width
    data.extend_from_slice(&[0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55]); // Pattern
    data.push(0x0A); // LF

    let result = send_escpos_data(&data).await;
    assert!(result.is_ok(), "Should successfully send raster graphics");
}

#[tokio::test]
#[ignore] // Ignored by default - requires running server
async fn test_status_query() {
    let mut stream = connect_to_server().await.expect("Should connect to server");

    // Send DLE EOT status query
    stream
        .write_all(b"\x10\x04\x01")
        .await
        .expect("Should send status query");
    stream.flush().await.expect("Should flush");

    // Try to read status response
    let mut response = [0u8; 1];
    let result =
        tokio::time::timeout(Duration::from_secs(1), stream.read_exact(&mut response)).await;

    // This will currently fail until we implement status responses
    assert!(
        result.is_ok(),
        "Should receive status response within timeout"
    );

    if let Ok(Ok(_)) = result {
        // Typical online status is 0x12
        println!("Received status: 0x{:02X}", response[0]);
    }
}

#[tokio::test]
#[ignore] // Ignored by default - requires running server
async fn test_multiple_connections() {
    // Test that server can handle multiple simultaneous connections
    let stream1 = connect_to_server().await;
    let stream2 = connect_to_server().await;

    assert!(stream1.is_ok(), "First connection should succeed");
    assert!(stream2.is_ok(), "Second connection should succeed");
}

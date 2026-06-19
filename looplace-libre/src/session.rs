//! FreeStyle session protocol: framing, encryption handshake, command/response.
//!
//! Ported from freestyle-hid's `_session.py` (Apache-2.0). The session is generic
//! over a [`HidTransport`] so the handshake can be validated offline against
//! recorded chatter (see the tests) before running against a physical reader.
//!
//! The encrypted handshake and per-message encrypt-then-MAC require the Libre 2
//! key constants, compiled in only with the `libre2-keys` feature.

use crate::crypto::{Speck, SpeckCmac};
use crate::error::{LibreError, Result};
use crate::transport::HidTransport;

const INIT_COMMAND: u8 = 0x01;
const INIT_RESPONSE: u8 = 0x71;
const KEEPALIVE_RESPONSE: u8 = 0x22;
const UNKNOWN_MESSAGE_RESPONSE: u8 = 0x30;
const ENCRYPTION_SETUP_COMMAND: u8 = 0x14;
const ENCRYPTION_SETUP_RESPONSE: u8 = 0x33;

/// Message types that are always sent/received in the clear, even on the
/// encrypted protocol (mirrors `_ALWAYS_UNENCRYPTED_MESSAGES`).
const ALWAYS_UNENCRYPTED: [u8; 13] = [
    INIT_COMMAND,
    0x04,
    0x05,
    0x06,
    0x0C,
    0x0D,
    ENCRYPTION_SETUP_COMMAND,
    0x15,
    ENCRYPTION_SETUP_RESPONSE,
    0x34,
    0x35,
    INIT_RESPONSE,
    KEEPALIVE_RESPONSE,
];

fn is_always_unencrypted(message_type: u8) -> bool {
    ALWAYS_UNENCRYPTED.contains(&message_type)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Read an unsigned big-endian integer from up to 8 bytes.
fn be_uint(bytes: &[u8]) -> u64 {
    let mut value = 0u64;
    for &b in bytes {
        value = (value << 8) | b as u64;
    }
    value
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// A protocol session with a FreeStyle reader over some [`HidTransport`].
pub struct Session<T: HidTransport> {
    transport: T,
    text_message_type: u8,
    text_reply_message_type: u8,
    encrypted_protocol: bool,
    crypt_enc: Option<Speck>,
    crypt_mac: Option<SpeckCmac>,
}

impl<T: HidTransport> Session<T> {
    /// Create a session. `text_message_type`/`text_reply_message_type` are the
    /// device-specific framing types for ASCII commands (e.g. `0x60`/`0x60` for
    /// the Libre family — wired up by the device driver in Phase 2).
    pub fn new(
        transport: T,
        text_message_type: u8,
        text_reply_message_type: u8,
        encrypted: bool,
    ) -> Self {
        Self {
            transport,
            text_message_type,
            text_reply_message_type,
            encrypted_protocol: encrypted,
            crypt_enc: None,
            crypt_mac: None,
        }
    }

    /// Consume the session and return the underlying transport.
    pub fn into_transport(self) -> T {
        self.transport
    }

    // ---- framing -----------------------------------------------------------

    /// Build a 64-byte `_FREESTYLE_MESSAGE`: type, length-prefixed command
    /// (padded to 55 bytes), 4 zero IV-counter bytes, 4 zero MAC bytes.
    fn build_message(message_type: u8, command: &[u8]) -> Result<[u8; 64]> {
        if command.len() > 54 {
            return Err(LibreError::Transport(format!(
                "command too long: {} bytes (max 54)",
                command.len()
            )));
        }
        let mut message = [0u8; 64];
        message[0] = message_type;
        message[1] = command.len() as u8;
        message[2..2 + command.len()].copy_from_slice(command);
        Ok(message)
    }

    /// Wrap a packet in a HID report (report-id byte 0 + content padded to 64).
    fn write_hid(&mut self, packet: &[u8]) -> Result<()> {
        let mut report = vec![0u8; 1 + 64];
        let n = packet.len().min(64);
        report[1..1 + n].copy_from_slice(&packet[..n]);
        self.transport.write(&report)
    }

    // ---- encryption --------------------------------------------------------

    fn encrypt_message(&self, packet: [u8; 64]) -> Result<[u8; 64]> {
        let enc = self
            .crypt_enc
            .as_ref()
            .ok_or_else(|| LibreError::Handshake("session not encrypted yet".into()))?;
        let mac = self
            .crypt_mac
            .as_ref()
            .ok_or_else(|| LibreError::Handshake("session not encrypted yet".into()))?;

        let mut output = packet;
        // 0xFF IV is actually 0, because of some weird padding (per the oracle).
        let encrypted = enc.encrypt(0xFF, &packet[1..56]);
        output[1..56].copy_from_slice(&encrypted);
        output[56..60].copy_from_slice(&[0u8; 4]);
        let signature = mac.sign(&output[0..60]);
        output[60..64].copy_from_slice(&signature.to_le_bytes()[4..8]);
        Ok(output)
    }

    fn decrypt_message(&self, packet: &[u8]) -> Result<[u8; 64]> {
        if packet.len() < 64 {
            return Err(LibreError::Parse(format!(
                "short encrypted packet: {} bytes",
                packet.len()
            )));
        }
        let enc = self
            .crypt_enc
            .as_ref()
            .ok_or_else(|| LibreError::Handshake("session not encrypted yet".into()))?;
        let mac = self
            .crypt_mac
            .as_ref()
            .ok_or_else(|| LibreError::Handshake("session not encrypted yet".into()))?;

        let signature = mac.sign(&packet[..60]);
        if signature.to_le_bytes()[4..8] != packet[60..64] {
            return Err(LibreError::Handshake("incoming message MAC mismatch".into()));
        }
        let iv = (be_uint(&packet[56..60])) << 8;
        let mut output = [0u8; 64];
        output.copy_from_slice(&packet[..64]);
        let plain = enc.decrypt(iv, &packet[1..56]);
        output[1..56].copy_from_slice(&plain);
        Ok(output)
    }

    // ---- commands ----------------------------------------------------------

    /// Send a raw command. Encrypts when the protocol is encrypted and the type
    /// is not in [`ALWAYS_UNENCRYPTED`].
    pub fn send_command(&mut self, message_type: u8, command: &[u8]) -> Result<()> {
        let mut message = Self::build_message(message_type, command)?;
        if self.encrypted_protocol && !is_always_unencrypted(message_type) {
            message = self.encrypt_message(message)?;
        }
        self.write_hid(&message)
    }

    /// Read one logical response, transparently skipping keepalives and mapping
    /// the documented error responses. Returns `(message_type, content)`.
    pub fn read_response(&mut self) -> Result<(u8, Vec<u8>)> {
        loop {
            let raw = self.transport.read()?;
            if raw.is_empty() {
                return Err(LibreError::Transport("empty HID read".into()));
            }
            let message_type = raw[0];

            let packet = if self.encrypted_protocol && !is_always_unencrypted(message_type) {
                self.decrypt_message(&raw)?.to_vec()
            } else {
                raw
            };

            if packet.len() < 2 {
                return Err(LibreError::Parse("response shorter than 2 bytes".into()));
            }
            let length = packet[1] as usize;
            let end = (2 + length).min(packet.len());
            let content = packet[2..end].to_vec();

            // Stray "22 01 xx" keepalive messages: ignore and read the next.
            if message_type == KEEPALIVE_RESPONSE {
                continue;
            }
            if message_type == UNKNOWN_MESSAGE_RESPONSE && content == [0x85] {
                return Err(LibreError::Handshake("invalid command".into()));
            }
            if message_type == ENCRYPTION_SETUP_RESPONSE && content == [0x15] {
                return Err(LibreError::Handshake("device encryption not initialized".into()));
            }
            if message_type == ENCRYPTION_SETUP_RESPONSE && content == [0x14] {
                return Err(LibreError::Handshake(
                    "device encryption initialization failed".into(),
                ));
            }

            return Ok((message_type, content));
        }
    }

    // ---- handshake ---------------------------------------------------------

    /// Run the encrypted handshake using a caller-supplied host nonce. Used
    /// directly by tests/replay for determinism; `connect` supplies an OS-random
    /// nonce for real devices.
    pub fn encryption_handshake_with_nonce(&mut self, host_nonce: [u8; 8]) -> Result<()> {
        #[cfg(not(feature = "libre2-keys"))]
        {
            let _ = host_nonce;
            Err(LibreError::Handshake(
                "encrypted handshake requires the `libre2-keys` feature".into(),
            ))
        }

        #[cfg(feature = "libre2-keys")]
        {
            use looplace_libre_keys as keys;

            // 1. Ask for the serial number (drives per-device key derivation).
            self.send_command(0x05, b"")?;
            let (response_type, serial_bytes) = self.read_response()?;
            if response_type != 0x06 {
                return Err(LibreError::Handshake(format!(
                    "unexpected serial response type {response_type:02x}"
                )));
            }
            let serial = &serial_bytes[..serial_bytes.len().min(13)];

            let auth_enc_key = SpeckCmac::new(keys::AUTHORIZATION_ENCRYPTION_KEY)
                .derive(b"AuthrEnc", serial);
            let auth_enc = Speck::new(auth_enc_key);
            let auth_mac_key =
                SpeckCmac::new(keys::AUTHORIZATION_MAC_KEY).derive(b"AuthrMAC", serial);
            let auth_mac = SpeckCmac::new(auth_mac_key);

            // 2. Request the challenge.
            self.send_command(ENCRYPTION_SETUP_COMMAND, b"\x11")?;
            let (response_type, challenge) = self.read_response()?;
            if response_type != ENCRYPTION_SETUP_RESPONSE {
                return Err(LibreError::Handshake(format!(
                    "unexpected challenge response type {response_type:02x}"
                )));
            }
            if challenge.len() < 16 || challenge[0] != 0x16 {
                return Err(LibreError::Parse(format!(
                    "malformed challenge: {}",
                    to_hex(&challenge)
                )));
            }
            let reader_nonce = &challenge[1..9];
            let challenge_iv = be_uint(&challenge[9..16]);

            // 3. Answer the challenge: encrypt(reader_nonce || host_nonce), MAC it.
            let mut nonces = Vec::with_capacity(16);
            nonces.extend_from_slice(reader_nonce);
            nonces.extend_from_slice(&host_nonce);
            let encrypted_challenge = auth_enc.encrypt(challenge_iv, &nonces);

            let mut raw_response = Vec::with_capacity(28);
            raw_response.extend_from_slice(&[ENCRYPTION_SETUP_COMMAND, 0x1A, 0x17]);
            raw_response.extend_from_slice(&encrypted_challenge);
            raw_response.push(0x01);
            let response_mac = auth_mac.sign(&raw_response);
            raw_response.extend_from_slice(&response_mac.to_le_bytes());
            self.write_hid(&raw_response)?;

            // 4. Verify the device's acceptance.
            let (response_type, acceptance) = self.read_response()?;
            if response_type != ENCRYPTION_SETUP_RESPONSE {
                return Err(LibreError::Handshake(format!(
                    "unexpected acceptance response type {response_type:02x}"
                )));
            }
            if acceptance.len() < 32 || acceptance[0] != 0x18 {
                return Err(LibreError::Parse(format!(
                    "malformed acceptance: {}",
                    to_hex(&acceptance)
                )));
            }
            let encrypted_nonces = &acceptance[1..17];
            let acceptance_iv = be_uint(&acceptance[17..24]);
            let acceptance_mac =
                u64::from_le_bytes(acceptance[24..32].try_into().expect("8 bytes"));

            // MAC is computed over the reconstructed header + first 24 content bytes.
            let mut mac_input = Vec::with_capacity(26);
            mac_input.extend_from_slice(&[ENCRYPTION_SETUP_RESPONSE, 0x22]);
            mac_input.extend_from_slice(&acceptance[..24]);
            if auth_mac.sign(&mac_input) != acceptance_mac {
                return Err(LibreError::Handshake("challenge acceptance MAC mismatch".into()));
            }

            let decoded = auth_enc.decrypt(acceptance_iv, encrypted_nonces);
            let mut expected = host_nonce.to_vec();
            expected.extend_from_slice(reader_nonce);
            if decoded != expected {
                return Err(LibreError::Handshake(
                    "decrypted nonces do not match expectation".into(),
                ));
            }

            // 5. Derive the session keys from serial || reader_nonce || host_nonce.
            let mut context_key = Vec::with_capacity(serial.len() + 16);
            context_key.extend_from_slice(serial);
            context_key.extend_from_slice(reader_nonce);
            context_key.extend_from_slice(&host_nonce);

            let ses_enc_key =
                SpeckCmac::new(keys::SESSION_ENCRYPTION_KEY).derive(b"SessnEnc", &context_key);
            let ses_mac_key =
                SpeckCmac::new(keys::SESSION_MAC_KEY).derive(b"SessnMAC", &context_key);
            self.crypt_enc = Some(Speck::new(ses_enc_key));
            self.crypt_mac = Some(SpeckCmac::new(ses_mac_key));

            Ok(())
        }
    }

    /// Open the connection (handshake if encrypted, then the init knock) using a
    /// caller-supplied host nonce.
    pub fn connect_with_nonce(&mut self, host_nonce: [u8; 8]) -> Result<()> {
        if self.encrypted_protocol {
            self.encryption_handshake_with_nonce(host_nonce)?;
        }
        self.send_command(INIT_COMMAND, b"")?;
        let (response_type, content) = self.read_response()?;
        if response_type != INIT_RESPONSE || content != [0x01] {
            return Err(LibreError::Handshake(format!(
                "unexpected init reply {response_type:02x}:{}",
                to_hex(&content)
            )));
        }
        Ok(())
    }

    /// Open the connection with an OS-random host nonce (real-device path).
    #[cfg(feature = "transport")]
    pub fn connect(&mut self) -> Result<()> {
        let mut host_nonce = [0u8; 8];
        getrandom::getrandom(&mut host_nonce)
            .map_err(|e| LibreError::Handshake(format!("nonce generation failed: {e}")))?;
        self.connect_with_nonce(host_nonce)
    }

    // ---- text commands -----------------------------------------------------

    fn send_text_command_raw(&mut self, command: &[u8]) -> Result<Vec<u8>> {
        self.send_command(self.text_message_type, command)?;

        let mut full = Vec::new();
        loop {
            let (message_type, content) = self.read_response()?;
            if message_type != self.text_reply_message_type {
                return Err(LibreError::Handshake(format!(
                    "unexpected text reply type {message_type:02x}: {}",
                    to_hex(&content)
                )));
            }
            full.extend_from_slice(&content);
            if find_subslice(&full, b"CMD OK").is_some()
                || find_subslice(&full, b"CMD Fail!").is_some()
            {
                break;
            }
        }

        // Expected tail: <message>CKSM:XXXXXXXX\r\nCMD OK\r\n  (or "CMD Fail!").
        let cksm_pos = find_subslice(&full, b"CKSM:")
            .ok_or_else(|| LibreError::Parse(format!("no checksum in reply: {}", to_hex(&full))))?;
        let message = full[..cksm_pos].to_vec();
        let checksum_hex = full
            .get(cksm_pos + 5..cksm_pos + 13)
            .ok_or_else(|| LibreError::Parse("truncated checksum".into()))?;
        let ok = find_subslice(&full[cksm_pos..], b"CMD OK").is_some();

        verify_checksum(&message, checksum_hex)?;
        if !ok {
            return Err(LibreError::Handshake("device reported command failure".into()));
        }
        Ok(message)
    }

    /// Send a text command and return the (lossily decoded) reply text.
    pub fn send_text_command(&mut self, command: &[u8]) -> Result<String> {
        let message = self.send_text_command_raw(command)?;
        Ok(String::from_utf8_lossy(&message).into_owned())
    }

    /// Query a "multirecord" reply (events/readings/history) into rows of fields.
    pub fn query_multirecord(&mut self, command: &[u8]) -> Result<Vec<Vec<String>>> {
        let message = self.send_text_command_raw(command)?;
        if message == b"Log Empty\r\n" {
            return Ok(Vec::new());
        }

        // Trailer: "<records...>\r\n<count>,<CHECKSUM>\r\n".
        let trimmed = message
            .strip_suffix(b"\r\n")
            .ok_or_else(|| LibreError::Parse("multirecord missing trailer".into()))?;
        let boundary = rfind_subslice(trimmed, b"\r\n")
            .ok_or_else(|| LibreError::Parse("multirecord missing count line".into()))?;
        let records_raw = &message[..boundary + 2];
        let count_line = &trimmed[boundary + 2..];

        let comma = find_subslice(count_line, b",")
            .ok_or_else(|| LibreError::Parse("multirecord count line malformed".into()))?;
        let checksum_hex = &count_line[comma + 1..];
        verify_checksum(records_raw, checksum_hex)?;

        let records_str = String::from_utf8_lossy(records_raw);
        let rows = records_str
            .split("\r\n")
            .filter(|line| !line.is_empty())
            .map(|line| line.split(',').map(|f| f.to_string()).collect())
            .collect();
        Ok(rows)
    }
}

fn rfind_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len())
        .rev()
        .find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// Verify the FreeStyle simple checksum (sum of message bytes) against the hex.
fn verify_checksum(message: &[u8], expected_hex: &[u8]) -> Result<()> {
    let hex = std::str::from_utf8(expected_hex)
        .map_err(|_| LibreError::Parse("non-utf8 checksum".into()))?;
    let expected = u64::from_str_radix(hex.trim(), 16)
        .map_err(|_| LibreError::Parse(format!("bad checksum hex: {hex}")))?;
    let calculated: u64 = message.iter().map(|&b| b as u64).sum();
    if calculated != expected {
        return Err(LibreError::Parse(format!(
            "checksum mismatch: expected {expected}, calculated {calculated}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::ReplayTransport;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// A 64-byte device input report carrying `content` (type, length, payload…).
    fn report(content: &[u8]) -> Vec<u8> {
        let mut v = vec![0u8; 64];
        v[..content.len()].copy_from_slice(content);
        v
    }

    #[test]
    fn build_and_frame_unencrypted_command() {
        let mut session = Session::new(ReplayTransport::new([]), 0x60, 0x60, false);
        session.send_command(INIT_COMMAND, b"").unwrap();
        let written = &session.transport.written()[0];
        assert_eq!(written.len(), 65);
        assert_eq!(written[0], 0x00); // report id
        assert_eq!(written[1], INIT_COMMAND); // message type
        assert_eq!(written[2], 0x00); // command length
        assert!(written[3..].iter().all(|&b| b == 0));
    }

    #[test]
    fn read_response_parses_type_and_content() {
        let transport = ReplayTransport::new([report(&[INIT_RESPONSE, 0x01, 0x01])]);
        let mut session = Session::new(transport, 0x60, 0x60, false);
        let (ty, content) = session.read_response().unwrap();
        assert_eq!(ty, INIT_RESPONSE);
        assert_eq!(content, vec![0x01]);
    }

    #[test]
    fn read_response_skips_keepalive() {
        let transport = ReplayTransport::new([
            report(&[KEEPALIVE_RESPONSE, 0x01, 0x00]),
            report(&[INIT_RESPONSE, 0x01, 0x01]),
        ]);
        let mut session = Session::new(transport, 0x60, 0x60, false);
        let (ty, _) = session.read_response().unwrap();
        assert_eq!(ty, INIT_RESPONSE);
    }

    #[test]
    fn text_command_roundtrip_with_checksum() {
        // Build a reply whose checksum matches: message = "ABC\r\n".
        let message = b"ABC\r\n";
        let sum: u32 = message.iter().map(|&b| b as u32).sum();
        let reply = format!(
            "{}CKSM:{:08X}\r\nCMD OK\r\n",
            String::from_utf8_lossy(message),
            sum
        );
        let bytes = reply.into_bytes();
        let content = [&[0x60u8, bytes.len() as u8][..], &bytes].concat();
        let transport = ReplayTransport::new([report(&content)]);
        let mut session = Session::new(transport, 0x60, 0x60, false);
        let out = session.send_text_command(b"$dummy?").unwrap();
        assert_eq!(out, "ABC\r\n");
    }

    // Fixtures for the encrypted handshake, generated by the reference Python
    // (`_freestyle_encryption.py` + `freestyle_keys.libre2`). Fixed inputs:
    //   serial="ABC1234567890", reader_nonce=00..07, challenge_iv=0x11223344556677,
    //   host_nonce=10..17, acceptance_iv=0x33445566778899.
    #[cfg(feature = "libre2-keys")]
    const HOST_NONCE: [u8; 8] = [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17];

    #[cfg(feature = "libre2-keys")]
    fn handshake_reports() -> Vec<Vec<u8>> {
        let serial = b"ABC1234567890";
        let serial_content = [&[0x06u8, serial.len() as u8][..], serial].concat();
        let challenge = unhex("16000102030405060711223344556677"); // 16 bytes
        let challenge_content =
            [&[ENCRYPTION_SETUP_RESPONSE, challenge.len() as u8][..], &challenge].concat();
        let acceptance =
            unhex("18e7dc12bbe3247b21d7f554564e7dd16133445566778899fab80bc0a1167e03"); // 32 bytes
        let acceptance_content =
            [&[ENCRYPTION_SETUP_RESPONSE, acceptance.len() as u8][..], &acceptance].concat();
        vec![
            report(&serial_content),
            report(&challenge_content),
            report(&acceptance_content),
        ]
    }

    // Full encrypted handshake driven over a replay transport, validated against
    // the reference Python.
    #[cfg(feature = "libre2-keys")]
    #[test]
    fn encrypted_handshake_matches_python_oracle() {
        let transport = ReplayTransport::new(handshake_reports());
        let mut session = Session::new(transport, 0x60, 0x60, true);
        session.encryption_handshake_with_nonce(HOST_NONCE).unwrap();

        // Third write is the challenge response; assert its 28 content bytes.
        let raw_response = &session.transport.written()[2];
        let expected_raw = unhex("141a1728d31034ede12616e986420bb395daee01e45bb5237289b295");
        assert_eq!(&raw_response[1..1 + expected_raw.len()], &expected_raw[..]);

        // The derived session keys must reproduce the oracle's encrypt_message.
        let mut packet = [0u8; 64];
        packet[0] = 0x60;
        packet[1] = 0x03;
        packet[2..5].copy_from_slice(b"abc");
        let encrypted = session.encrypt_message(packet).unwrap();
        let expected = unhex(
            "6005877a4611031da465e052c2b680e7d1d73543c7e01c13de34771570f5fddea53320caa32ec5821b5dabeb78cf8578d636c910c1c0342200000000c0230c83",
        );
        assert_eq!(&encrypted[..], &expected[..]);
    }

    // The public entry point: handshake, then the init knock and its 0x71 reply.
    #[cfg(feature = "libre2-keys")]
    #[test]
    fn connect_with_nonce_completes() {
        let mut reads = handshake_reports();
        reads.push(report(&[INIT_RESPONSE, 0x01, 0x01]));
        let mut session = Session::new(ReplayTransport::new(reads), 0x60, 0x60, true);
        session.connect_with_nonce(HOST_NONCE).unwrap();
    }

    // `decrypt_message` handles *incoming* (device→host) messages, which use the
    // `iv_counter` field rather than the outgoing 0xFF IV — so it is not the
    // inverse of `encrypt_message`. Validate it against a device-side message
    // built by the reference Python with the same derived session keys.
    #[test]
    fn decrypt_message_matches_python_oracle() {
        // Session keys derived by the oracle (not the gray-artifact device keys).
        let ses_enc_key = 0x930747dd6497be528d6d321f3f2b7931u128;
        let ses_mac_key = 0x9a4f0991e3cfc2718ce4373a2bccfa37u128;
        let mut session = Session::new(ReplayTransport::new([]), 0x60, 0x60, true);
        session.crypt_enc = Some(Speck::new(ses_enc_key));
        session.crypt_mac = Some(SpeckCmac::new(ses_mac_key));

        let incoming = unhex("60842472eaa474f278d92a1a1b8a5e957ecd69c2751f25a1a57907a448b141003c3ad0986edd0bef84f876c51e01f1dfef98e89697dedc7a000000014eb39b3f");
        let decrypted = session.decrypt_message(&incoming).unwrap();
        // Recovered plaintext header: type 0x60, len 5, "hello".
        assert_eq!(&decrypted[..7], &unhex("600568656c6c6f")[..]);

        // The full encrypted-protocol read path decrypts and parses too.
        let transport = ReplayTransport::new([incoming]);
        let mut session = Session::new(transport, 0x60, 0x60, true);
        session.crypt_enc = Some(Speck::new(ses_enc_key));
        session.crypt_mac = Some(SpeckCmac::new(ses_mac_key));
        let (ty, content) = session.read_response().unwrap();
        assert_eq!(ty, 0x60);
        assert_eq!(content, b"hello");
    }
}

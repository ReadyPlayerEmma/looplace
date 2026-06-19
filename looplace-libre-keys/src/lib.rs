// SPDX-License-Identifier: 0BSD
//
// Ported from the `freestyle-keys` project (freestyle_keys/libre2.py).

//! Encryption keys used by FreeStyle Libre 2 glucometers.
//!
//! These constants are key material required to communicate with the FreeStyle
//! Libre 2 reader. Per the upstream `freestyle-keys` project:
//!
//! > No copyright is claimed for these keys, as they are requisite constants
//! > that are part of the device's communication protocol. We assert these keys
//! > do not constitute Technical Protection Measures for the purpose of
//! > copyright protection, as they do not protect copyrighted material but
//! > rather obfuscate the communication between two devices under the control of
//! > the user.
//!
//! This crate is deliberately separate and optional: `looplace-libre` depends on
//! it only behind its `libre2-keys` feature, so the driver crate and any default
//! build remain free of the keys.

/// Authorization-phase encryption key (derives the per-serial `AuthrEnc` key).
pub const AUTHORIZATION_ENCRYPTION_KEY: u128 = 0x360C0E171551821D7961F891197B52A1;
/// Authorization-phase MAC key (derives the per-serial `AuthrMAC` key).
pub const AUTHORIZATION_MAC_KEY: u128 = 0x738F004CD1A80D16622DB2E0DB8C60D4;
/// Session-phase encryption key (derives the per-session `SessnEnc` key).
pub const SESSION_ENCRYPTION_KEY: u128 = 0x9D42333D9DDD20A7164C2AB057F92EFD;
/// Session-phase MAC key (derives the per-session `SessnMAC` key).
pub const SESSION_MAC_KEY: u128 = 0x12B0D868D117D7C8379DE50FA97A7BA0;

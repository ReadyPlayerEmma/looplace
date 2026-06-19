// SPDX-License-Identifier: Apache-2.0
//
// Rust port of the FreeStyle Libre 2 session cryptography:
//   * Speck 64/128 block cipher (32-bit words, alpha=8, beta=3)
//   * CTR-mode stream encryption (decrypt == encrypt)
//   * Speck-CMAC and the CMAC-based key-derivation function
//
// Ported clean from freestyle-hid's `_freestyle_encryption.py` (Apache-2.0).
// No external crates; std only.

//! Libre 2 session cryptography (Speck 64/128 + CTR + Speck-CMAC + CMAC-KDF).
//!
//! This is a clean, dependency-free port verified byte-for-byte against the
//! canonical Python reference. See `examples/selfcheck.rs` for the reference
//! vectors and `#[cfg(test)]` below for the frozen known-answer tests.

/// Speck 64/128 block cipher with CTR-mode helper.
pub struct Speck {
    round_keys: Vec<u32>,
}

impl Speck {
    pub fn new(key: u128) -> Self {
        let m = 0xFFFF_FFFFu128;
        let mut round_keys = vec![(key & m) as u32];
        let mut key_buf: Vec<u32> = (1..4).map(|x| ((key >> (x * 32)) & m) as u32).collect();
        for x in 0..26u32 {
            let (a, b) = Self::enc_round(key_buf[x as usize], round_keys[x as usize], x);
            key_buf.push(a);
            round_keys.push(b);
        }
        Speck { round_keys }
    }

    #[inline]
    fn enc_round(x: u32, y: u32, k: u32) -> (u32, u32) {
        let x = x.rotate_right(8).wrapping_add(y) ^ k; // ((x>>>8)+y) ^ k
        let y = y.rotate_left(3) ^ x; // (y<<<3) ^ x
        (x, y)
    }

    #[inline]
    fn dec_round(x: u32, y: u32, k: u32) -> (u32, u32) {
        let new_y = (x ^ y).rotate_right(3);
        let new_x = ((x ^ k).wrapping_sub(new_y)).rotate_left(8);
        (new_x, new_y)
    }

    pub fn encrypt_block(&self, plain: u64) -> u64 {
        let (mut x, mut y) = ((plain >> 32) as u32, plain as u32);
        for &k in &self.round_keys {
            let (nx, ny) = Self::enc_round(x, y, k);
            x = nx;
            y = ny;
        }
        ((x as u64) << 32) | y as u64
    }

    pub fn decrypt_block(&self, enc: u64) -> u64 {
        let (mut x, mut y) = ((enc >> 32) as u32, enc as u32);
        for &k in self.round_keys.iter().rev() {
            let (nx, ny) = Self::dec_round(x, y, k);
            x = nx;
            y = ny;
        }
        ((x as u64) << 32) | y as u64
    }

    /// CTR-mode stream cipher. Decryption is identical to encryption.
    // Kept byte-for-byte faithful to the Python-verified port; `repeat().take()`
    // avoids raising the MSRV for `repeat_n` and keeps the diff against the oracle clean.
    #[allow(clippy::manual_repeat_n)]
    pub fn crypt(&self, iv: u64, data: &[u8]) -> Vec<u8> {
        let input_len = data.len();
        let mut buf = data.to_vec();
        buf.extend(std::iter::repeat(0u8).take(8 - (input_len % 8)));
        let mut counter = iv.swap_bytes();
        let mut out = Vec::with_capacity(buf.len());
        for chunk in buf.chunks_exact(8) {
            let keystream = self.encrypt_block(counter);
            let block = u64::from_le_bytes(chunk.try_into().unwrap());
            out.extend_from_slice(&(keystream ^ block).to_le_bytes());
            counter = counter.wrapping_add(1);
        }
        out.truncate(input_len);
        out
    }

    /// CTR encryption. Alias for [`Speck::crypt`]; mirrors the Python
    /// `SpeckEncrypt.encrypt` so the session port reads like the oracle.
    pub fn encrypt(&self, iv: u64, data: &[u8]) -> Vec<u8> {
        self.crypt(iv, data)
    }

    /// CTR decryption — identical to encryption for a stream cipher.
    pub fn decrypt(&self, iv: u64, data: &[u8]) -> Vec<u8> {
        self.crypt(iv, data)
    }
}

/// CMAC over Speck, plus the SP800-108-style KDF the session handshake uses.
pub struct SpeckCmac {
    cipher: Speck,
    k1: u64,
    k2: u64,
}

impl SpeckCmac {
    pub fn new(key: u128) -> Self {
        let cipher = Speck::new(key);
        let k0 = cipher.encrypt_block(0).swap_bytes();
        let mut k1 = k0 << 1;
        if k0 >> 63 != 0 {
            k1 ^= 0x1b;
        }
        let mut k2 = k1 << 1;
        if k1 >> 63 != 0 {
            k2 ^= 0x1b;
        }
        SpeckCmac {
            cipher,
            k1: k1.swap_bytes(),
            k2: k2.swap_bytes(),
        }
    }

    pub fn sign(&self, data: &[u8]) -> u64 {
        let mut c = 0u64;
        let n = data.len();
        let mut i = 0;
        while i < n {
            let left = n - i;
            let block = if left == 8 {
                u64::from_le_bytes(data[i..i + 8].try_into().unwrap()) ^ self.k1
            } else if left < 8 {
                let mut b = [0u8; 8];
                b[..left].copy_from_slice(&data[i..i + left]);
                b[left] = 0x80;
                u64::from_le_bytes(b) ^ self.k2
            } else {
                u64::from_le_bytes(data[i..i + 8].try_into().unwrap())
            };
            c = self.cipher.encrypt_block(c ^ block);
            i += 8;
        }
        c
    }

    pub fn derive(&self, label: &[u8], context: &[u8]) -> u128 {
        let mut data = Vec::new();
        data.extend_from_slice(label);
        data.push(0x00);
        data.extend_from_slice(context);
        data.extend_from_slice(&[0x80, 0x00]);
        let mut m1 = vec![0x01u8];
        m1.extend_from_slice(&data);
        let mut m2 = vec![0x02u8];
        m2.extend_from_slice(&data);
        let d1 = self.sign(&m1) as u128;
        let d2 = (self.sign(&m2) as u128) << 64;
        d1 | d2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors only. These two are *session* keys used by the Python
    // reference's self-check — not the four handshake authorization keys (the
    // licensing gray artifact), which live in a separate optional keys crate.
    const SESSION_ENCRYPTION_KEY: u128 = 0x9D42333D9DDD20A7164C2AB057F92EFD;
    const SESSION_MAC_KEY: u128 = 0x12B0D868D117D7C8379DE50FA97A7BA0;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn block_roundtrips() {
        let c = Speck::new(SESSION_ENCRYPTION_KEY);
        for pt in [0u64, 1, 0x0123456789abcdef, u64::MAX] {
            assert_eq!(c.decrypt_block(c.encrypt_block(pt)), pt);
        }
    }

    #[test]
    fn ctr_roundtrips() {
        let c = Speck::new(SESSION_ENCRYPTION_KEY);
        for msg in [&b""[..], b"$history?", b"a", b"12345678", b"123456789"] {
            let ct = c.crypt(42, msg);
            assert_eq!(c.crypt(42, &ct), msg);
        }
    }

    // Known-answer tests frozen from the Python-verified reference output
    // (freestyle-hid `_freestyle_encryption.py`). Reproduce with:
    //   cargo run -p looplace-libre --example selfcheck
    #[test]
    fn known_answer_vectors() {
        let c = Speck::new(SESSION_ENCRYPTION_KEY);
        assert_eq!(format!("{:016x}", c.encrypt_block(0)), "d1818e18246d376e");
        assert_eq!(
            format!("{:016x}", c.encrypt_block(0x0123456789abcdef)),
            "48c376a3d23c1110"
        );
        assert_eq!(hex(&c.crypt(42, b"$history?")), "9d56f3c186102ad917");

        let m = SpeckCmac::new(SESSION_MAC_KEY);
        assert_eq!(format!("{:016x}", m.sign(b"hello world!")), "305e4e71e48d06ca");
        assert_eq!(
            format!("{:032x}", m.derive(b"authz", &[1, 2, 3, 4])),
            "cd036e73147345dede97532520caaa40"
        );
    }
}

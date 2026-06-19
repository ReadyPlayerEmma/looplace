// SPDX-License-Identifier: Apache-2.0
//
// Self-check harness for the Libre 2 session crypto. Prints the same vectors the
// canonical Python reference produced, so the port can be diffed byte-for-byte.
//
//   cargo run -p looplace-libre --example selfcheck

use looplace_libre::crypto::{Speck, SpeckCmac};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    const SESSION_ENCRYPTION_KEY: u128 = 0x9D42333D9DDD20A7164C2AB057F92EFD;
    const SESSION_MAC_KEY: u128 = 0x12B0D868D117D7C8379DE50FA97A7BA0;

    let c = Speck::new(SESSION_ENCRYPTION_KEY);
    println!("block_enc(0)\t{:016x}", c.encrypt_block(0));
    println!(
        "block_enc(0x0123456789abcdef)\t{:016x}",
        c.encrypt_block(0x0123456789abcdef)
    );
    println!(
        "block_dec(block_enc(X))\t{:016x}",
        c.decrypt_block(c.encrypt_block(0x0123456789abcdef))
    );
    let ct = c.crypt(42, b"$history?");
    println!("ctr_enc(42,'$history?')\t{}", hex(&ct));
    println!(
        "ctr_roundtrip\t{}",
        String::from_utf8(c.crypt(42, &ct)).unwrap()
    );

    let m = SpeckCmac::new(SESSION_MAC_KEY);
    println!("cmac.sign('hello world!')\t{:016x}", m.sign(b"hello world!"));
    println!(
        "cmac.derive(lbl,ctx)\t{:032x}",
        m.derive(b"authz", &[1, 2, 3, 4])
    );
}

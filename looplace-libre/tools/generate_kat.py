# SPDX-License-Identifier: Apache-2.0
"""Dev-only oracle: regenerate the known-answer vectors baked into the Rust tests.

This is NOT part of the build (the binary stays pure Rust). It drives the
canonical Python reference as an oracle so the `looplace-libre` port can be
diffed byte-for-byte and kept regression-proof.

Requires the vendored reference packages (gitignored, local-only):
    .reference_packages/freestyle-hid/freestyle_hid/_freestyle_encryption.py

Run from the repo root:
    python3 looplace-libre/tools/generate_kat.py

The printed values appear as `assert_eq!` constants in:
    looplace-libre/src/crypto.rs   (crypto KATs)
    looplace-libre/src/session.rs  (handshake / encrypt / decrypt KATs)
"""

import importlib.util
import pathlib

REPO = pathlib.Path(__file__).resolve().parents[2]
ENC = REPO / ".reference_packages/freestyle-hid/freestyle_hid/_freestyle_encryption.py"

spec = importlib.util.spec_from_file_location("fenc", ENC)
fenc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fenc)
SpeckEncrypt, SpeckCMAC = fenc.SpeckEncrypt, fenc.SpeckCMAC

# Key constants (freestyle-keys/libre2.py — disclaimed protocol constants).
AUTH_ENC = 0x360C0E171551821D7961F891197B52A1
AUTH_MAC = 0x738F004CD1A80D16622DB2E0DB8C60D4
SES_ENC = 0x9D42333D9DDD20A7164C2AB057F92EFD
SES_MAC = 0x12B0D868D117D7C8379DE50FA97A7BA0

# Fixed, arbitrary stand-ins for live device values (deterministic tests).
SERIAL = b"ABC1234567890"               # response[1][:13]
READER_NONCE = bytes(range(8))          # 00..07
CHALLENGE_IV = 0x11223344556677         # 7-byte challenge IV
HOST_NONCE = bytes(range(0x10, 0x18))   # 10..17
ACCEPTANCE_IV = 0x33445566778899        # 7-byte acceptance IV


def h(b: bytes) -> str:
    return b.hex()


def main() -> None:
    # crypto.rs known-answer vectors
    c = SpeckEncrypt(SES_ENC)
    print("[crypto.rs]")
    print("  block_enc(0)            ", format(c.encrypt_block(0), "016x"))
    print("  block_enc(0x0123..ef)   ", format(c.encrypt_block(0x0123456789ABCDEF), "016x"))
    print("  ctr(42,'$history?')     ", h(c.encrypt(42, b"$history?")))
    m = SpeckCMAC(SES_MAC)
    print("  cmac.sign('hello world!')", format(m.sign(b"hello world!"), "016x"))
    print("  cmac.derive(authz,1234) ", format(m.derive(b"authz", bytes([1, 2, 3, 4])), "032x"))

    # session.rs handshake KATs
    context_key = SERIAL + READER_NONCE + HOST_NONCE
    auth_enc_key = SpeckCMAC(AUTH_ENC).derive(b"AuthrEnc", SERIAL)
    auth_mac_key = SpeckCMAC(AUTH_MAC).derive(b"AuthrMAC", SERIAL)
    auth_enc, auth_mac = SpeckEncrypt(auth_enc_key), SpeckCMAC(auth_mac_key)

    enc_chal = auth_enc.encrypt(CHALLENGE_IV, READER_NONCE + HOST_NONCE)
    raw_nomac = bytes([0x14, 0x1A, 0x17]) + enc_chal + bytes([0x01])
    raw_response = raw_nomac + auth_mac.sign(raw_nomac).to_bytes(8, "little")

    enc_nonces = auth_enc.encrypt(ACCEPTANCE_IV, HOST_NONCE + READER_NONCE)
    content24 = bytes([0x18]) + enc_nonces + ACCEPTANCE_IV.to_bytes(7, "big")
    acceptance = content24 + auth_mac.sign(bytes([0x33, 0x22]) + content24).to_bytes(8, "little")

    ses_enc_key = SpeckCMAC(SES_ENC).derive(b"SessnEnc", context_key)
    ses_mac_key = SpeckCMAC(SES_MAC).derive(b"SessnMAC", context_key)

    print("\n[session.rs handshake]")
    print("  challenge_content       ", h(bytes([0x16]) + READER_NONCE + CHALLENGE_IV.to_bytes(7, "big")))
    print("  acceptance_content      ", h(acceptance))
    print("  raw_response (write[2]) ", h(raw_response))

    # encrypt_message KAT (outgoing, IV 0xFF)
    ses_enc, ses_mac = SpeckEncrypt(ses_enc_key), SpeckCMAC(ses_mac_key)
    pkt = bytearray(64)
    pkt[0], pkt[1], pkt[2:5] = 0x60, 0x03, b"abc"
    out = bytearray(pkt)
    out[1:56] = ses_enc.encrypt(0xFF, bytes(pkt[1:56]))
    out[56:60] = bytes(4)
    out[60:64] = ses_mac.sign(bytes(out[0:60])).to_bytes(8, "little")[4:]
    print("  encrypt_message(0x60..) ", h(bytes(out)))

    # decrypt_message KAT (incoming, iv_counter field)
    pin = bytearray(64)
    pin[0], pin[1], pin[2:7] = 0x60, 0x05, b"hello"
    ivc = (1).to_bytes(4, "big")
    msg = bytearray(64)
    msg[0] = pin[0]
    msg[1:56] = ses_enc.encrypt(int.from_bytes(ivc, "big") << 8, bytes(pin[1:56]))
    msg[56:60] = ivc
    msg[60:64] = ses_mac.sign(bytes(msg[0:60])).to_bytes(8, "little")[4:]
    print("  incoming_ct (decrypt)   ", h(bytes(msg)))


if __name__ == "__main__":
    main()

// SPDX-License-Identifier: Apache-2.0
//! Live smoke test: connect to a physically attached FreeStyle Libre 2 **reader**
//! over USB and run a few read-only text commands. This is the Phase 1
//! "done when `connect()` round-trips a text command" check.
//!
//! Requires the `transport` and `libre2-keys` features and a connected reader:
//!
//! ```text
//! cargo run -p looplace-libre --features transport,libre2-keys --example connect_smoke
//! ```
//!
//! Only read-only commands are issued ($serlnum?, $swver?, $date?, $time?) — it
//! never writes to or modifies the device.

#[cfg(all(feature = "transport", feature = "libre2-keys"))]
fn main() {
    use looplace_libre::session::Session;
    use looplace_libre::transport::{HidApiTransport, USB_PRODUCT_ID_LIBRE2, USB_VENDOR_ID};

    println!("Looplace · FreeStyle Libre 2 connect smoke test");
    println!(
        "Opening USB HID device {USB_VENDOR_ID:04x}:{USB_PRODUCT_ID_LIBRE2:04x} …"
    );

    let transport = match HidApiTransport::open_libre2() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("✗ could not open the reader: {e}");
            eprintln!("  • Plug in the FreeStyle Libre 2 *reader* (the handheld), not just a sensor.");
            eprintln!("  • Make sure it's powered on and not mid-menu.");
            eprintln!("  • Linux: raw HID may need a udev rule or sudo.");
            std::process::exit(1);
        }
    };
    println!("✓ device opened");

    // Libre 2 speaks the encrypted protocol; text framing is 0x60/0x60.
    let mut session = Session::new(transport, 0x60, 0x60, true);

    print!("Encrypted handshake + init knock … ");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    match session.connect() {
        Ok(()) => println!("✓"),
        Err(e) => {
            println!("✗");
            eprintln!("handshake/connect failed: {e}");
            eprintln!("If this reproduces, capture chatter with freestyle-hid's");
            eprintln!("extract_chatter.py and we'll diff it offline via ReplayTransport.");
            std::process::exit(2);
        }
    }

    // Read-only identity/clock queries. Libre uses `$sn?` (the base-class
    // `$serlnum?` is incompatible) and `$uom?` for the glucose unit (0=mmol/L,
    // 1=mg/dL).
    let mut ok = true;
    for cmd in [&b"$sn?"[..], b"$swver?", b"$uom?", b"$date?", b"$time?"] {
        let label = String::from_utf8_lossy(cmd);
        match session.send_text_command(cmd) {
            Ok(reply) => println!("  {label:<10} → {}", reply.trim_end()),
            Err(e) => {
                ok = false;
                eprintln!("  {label:<10} ✗ {e}");
            }
        }
    }

    if ok {
        println!("\n🎉 Live round-trip succeeded — the native Rust driver is talking to your reader.");
    } else {
        println!("\nConnected, but a command failed — share the output and we'll dig in.");
        std::process::exit(3);
    }
}

#[cfg(not(all(feature = "transport", feature = "libre2-keys")))]
fn main() {
    eprintln!(
        "connect_smoke needs the `transport` and `libre2-keys` features and a connected reader:\n\
         \n    cargo run -p looplace-libre --features transport,libre2-keys --example connect_smoke\n"
    );
    std::process::exit(2);
}

use anyhow::Result;
use bytes::{BufMut, BytesMut};

fn main() -> Result<()> {
    let mut buf = BytesMut::with_capacity(1024);
    buf.extend_from_slice(b"hello world\n");
    buf.put(&b"goodbye world"[..]);
    buf.put_i64(0xdeadbeef); // inferred as i64
                             // 0xdeadbeef without explicit type annotation is a type-inferred literal. When passed to put_i64(), Rust infers it as i64:

    println!("{:?}", buf);
    // b"hello world\ngoodbye world\0\0\0\0\xde\xad\xbe\xef"
    // The \0\0\0\0 are zero bytes (null bytes) that come from padding.
    // Why Zero Bytes Appear
    // When you call buf.put_i64(0xdeadbeef), it writes an i64 (8 bytes), not just the 4 bytes of 0xdeadbeef.

    // Breaking It Down
    // 0xdeadbeef is a 32-bit value:
    // ├─ In hex: 0xdeadbeef
    // ├─ In decimal: 3,735,928,559
    // └─ Size: 4 bytes (32 bits)

    // But put_i64() writes 8 bytes (64 bits):
    // ├─ Upper 32 bits (padding): 0x00000000 (all zeros!)
    // └─ Lower 32 bits (actual value): 0xdeadbeef

    // put_i64(0xdeadbeef) writes these 8 bytes in big-endian:

    // Byte 1: 0x00  ← \0 (padding)
    // Byte 2: 0x00  ← \0 (padding)
    // Byte 3: 0x00  ← \0 (padding)
    // Byte 4: 0x00  ← \0 (padding)
    // Byte 5: 0xde  ← \xde
    // Byte 6: 0xad  ← \xad
    // Byte 7: 0xbe  ← \xbe
    // Byte 8: 0xef  ← \xef

    // Result: \0\0\0\0\xde\xad\xbe\xef
    // What is \0?
    // \0 is the null byte (byte value 0).

    let a = buf.split(); // Take ALL data, return it, clear buf
    let mut b = a.freeze(); // Convert to immutable Bytes

    let c = b.split_to(12); // Take first n bytes
    println!("{:?}", c);

    println!("{:?}", b);
    println!("{:?}", buf);

    Ok(())
}

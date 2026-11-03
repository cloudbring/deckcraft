//! Detect password-protected OOXML presentations using the shared CFB reader.

use craft_cfb::{Cfb, EntryKind};

const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const ENCRYPTED_STREAMS: [&str; 2] = ["EncryptedPackage", "EncryptionInfo"];

/// Require both encryption streams in a valid CFB. Preserve the UTF-16LE marker scan
/// for malformed containers only, so damaged password-protected files remain recognizable.
pub(crate) fn is_encrypted(bytes: &[u8]) -> bool {
    match Cfb::open(bytes) {
        Ok(cfb) => ENCRYPTED_STREAMS
            .iter()
            .all(|name| cfb.entries().any(|entry| entry.kind() == EntryKind::Stream && entry.path().eq_ignore_ascii_case(name))),
        Err(_) => bytes.starts_with(&SIGNATURE) && ENCRYPTED_STREAMS.iter().any(|name| contains_utf16(bytes, name)),
    }
}

fn contains_utf16(hay: &[u8], needle: &str) -> bool {
    let encoded: Vec<u8> = needle.encode_utf16().flat_map(u16::to_le_bytes).collect();
    !encoded.is_empty() && hay.windows(encoded.len()).any(|window| window == encoded.as_slice())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const END_OF_CHAIN: u32 = 0xFFFF_FFFE;

    /// A minimal valid compound file (512-byte sectors: FAT at sector 0, directory at sector 1)
    /// whose directory holds a root entry plus the given stream names.
    fn build(streams: &[&str]) -> Vec<u8> {
        let mut b = vec![0u8; 512 * 3];
        b[..8].copy_from_slice(&SIGNATURE);
        b[24..26].copy_from_slice(&0x3Eu16.to_le_bytes()); // minor version
        b[26..28].copy_from_slice(&3u16.to_le_bytes()); // major version
        b[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes()); // byte order
        b[30..32].copy_from_slice(&9u16.to_le_bytes()); // sector shift
        b[32..34].copy_from_slice(&6u16.to_le_bytes()); // mini sector shift
        b[44..48].copy_from_slice(&1u32.to_le_bytes()); // FAT sectors
        b[48..52].copy_from_slice(&1u32.to_le_bytes()); // first directory sector
        b[56..60].copy_from_slice(&4096u32.to_le_bytes());
        b[60..64].copy_from_slice(&END_OF_CHAIN.to_le_bytes()); // no mini FAT
        b[68..72].copy_from_slice(&END_OF_CHAIN.to_le_bytes()); // no DIFAT sectors
        for i in 0..109 {
            b[76 + i * 4..80 + i * 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        }
        b[76..80].copy_from_slice(&0u32.to_le_bytes()); // FAT lives in sector 0
        // FAT (sector 0, at byte 512): sector 0 is a FAT sector, sector 1 ends its chain.
        for i in 0..128 {
            b[512 + i * 4..516 + i * 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        }
        b[512..516].copy_from_slice(&0xFFFF_FFFDu32.to_le_bytes());
        b[516..520].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        // Directory (sector 1, at byte 1024).
        for (k, name) in std::iter::once("Root Entry").chain(streams.iter().copied()).take(4).enumerate() {
            let e = 1024 + k * 128;
            let units: Vec<u16> = name.encode_utf16().take(31).collect();
            for (i, u) in units.iter().enumerate() {
                b[e + i * 2..e + i * 2 + 2].copy_from_slice(&u.to_le_bytes());
            }
            b[e + 64..e + 66].copy_from_slice(&((units.len() as u16 + 1) * 2).to_le_bytes());
            b[e + 66] = if k == 0 { 5 } else { 2 };
            b[e + 67] = 1;
            b[e + 68..e + 80].fill(0xFF);
            if k == 0 && !streams.is_empty() {
                b[e + 76..e + 80].copy_from_slice(&1u32.to_le_bytes());
            } else if k > 0 && k < streams.len().min(3) {
                b[e + 72..e + 76].copy_from_slice(&(k as u32 + 1).to_le_bytes());
            }
            b[e + 116..e + 120].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        }
        b
    }

    #[test]
    fn reads_directory_names() {
        let b = build(&["EncryptionInfo", "EncryptedPackage"]);
        let cfb = Cfb::open(&b).unwrap();
        assert_eq!(cfb.root().kind(), EntryKind::Root);
        assert_eq!(cfb.entries().map(|entry| entry.path()).collect::<Vec<_>>(), ["", "EncryptionInfo", "EncryptedPackage"]);
        assert!(is_encrypted(&b));
        // A PowerPoint 97-2003 .ppt is a compound file too, without the encryption streams.
        assert!(!is_encrypted(&build(&["PowerPoint Document", "Current User"])));
        assert!(!is_encrypted(b"PK\x03\x04"));
    }

    #[test]
    fn valid_containers_require_both_root_streams() {
        for streams in [&["EncryptionInfo"][..], &["EncryptedPackage"], &["PowerPoint Document"]] {
            let bytes = build(streams);
            assert!(Cfb::open(&bytes).is_ok());
            assert!(!is_encrypted(&bytes));
        }
        assert!(is_encrypted(&build(&["encryptioninfo", "encryptedpackage"])));
        let mut bytes = build(&["EncryptionInfo", "EncryptedPackage"]);
        bytes[1024 + 128 + 66] = 1;
        assert!(Cfb::open(&bytes).is_ok());
        assert!(!is_encrypted(&bytes));
    }

    #[test]
    fn malformed_containers_keep_the_marker_scan_fallback() {
        for name in ENCRYPTED_STREAMS {
            let mut bytes = SIGNATURE.to_vec();
            bytes.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
            assert!(Cfb::open(&bytes).is_err());
            assert!(is_encrypted(&bytes));
            bytes[0] = 0;
            assert!(!is_encrypted(&bytes));
        }
        let mut bytes = build(&["PowerPoint Document"]);
        let marker: Vec<u8> = "EncryptionInfo".encode_utf16().flat_map(u16::to_le_bytes).collect();
        bytes[512 + 32..512 + 32 + marker.len()].copy_from_slice(&marker);
        assert!(Cfb::open(&bytes).is_ok());
        assert!(!is_encrypted(&bytes));
    }

    #[test]
    fn hostile_headers_never_panic() {
        let mut b = build(&["PowerPoint Document"]);
        for at in [30usize, 48, 68, 76, 512, 516, 1024 + 64] {
            for v in [0u32, 1, 7, 0xFFFF_FFFA, 0xFFFF_FFFF, 0x7FFF_FFFF] {
                let mut c = b.clone();
                c[at..at + 4].copy_from_slice(&v.to_le_bytes());
                let _ = is_encrypted(&c);
                for cut in [0usize, 8, 100, 600, 1100] {
                    let _ = is_encrypted(&c[..cut.min(c.len())]);
                }
            }
        }
        // A directory chain that loops back on itself stops at the bound.
        b[516..520].copy_from_slice(&1u32.to_le_bytes());
        assert!(!is_encrypted(&b));
    }

    /// #346: opening a password-protected presentation says so instead of failing as an unknown
    /// file.
    #[test]
    fn a_password_protected_presentation_is_named_as_such() {
        let locked = build(&["EncryptionInfo", "EncryptedPackage"]);
        let err = deckcraft_engine::cmd::file::open_presentation("Locked.pptx", &locked).unwrap_err().to_string();
        assert!(err.contains("Locked.pptx is password-protected"), "{err}");
        let ppt = build(&["PowerPoint Document", "Current User"]);
        let err = deckcraft_engine::cmd::file::open_presentation("Old.ppt", &ppt).unwrap_err().to_string();
        assert!(!err.contains("password"), "{err}");
    }
}

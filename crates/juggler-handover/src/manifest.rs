//! Artifact manifest primitives (AJ-E202): sha256 hashing and the
//! one manifest row shape shared by packs and edges. Also carries
//! the always-on credential guard — no pack file may contain
//! credential material regardless of policy (the full detection
//! matrix + user policy land with AJ-E208; the guard here is the
//! floor that can never be switched off).

use sha2::{Digest, Sha256};

/// One manifest row: a file the switch touched (or explicitly
/// dropped), hashed and dispositioned. Mirrors
/// `juggler_core::model::ManifestEntry` on the edge side; kept
/// separate so pack generation stays core-free on this shape.
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ManifestRow {
    /// Path relative to the pack root (e.g. `conversation.md`).
    pub rel_path: String,
    /// sha256 hex of the file's bytes.
    pub sha256: String,
    /// Size in bytes.
    pub bytes: u64,
    /// Artifact kind (`conversation`, `rules`, `diffs`, `state`, …).
    pub kind: String,
    /// Disposition: `copy | translate | regenerate | drop`.
    pub disposition: String,
}

/// sha256 hex of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// A credential-pattern hit (values are NEVER captured — kind only,
/// never the matched bytes).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CredentialHit {
    /// The detected kind (`private-key` | `api-key` | `bearer`).
    pub kind: &'static str,
}

/// The always-on credential guard: scans pack bytes for credential
/// material that must never ship in ANY pack under ANY policy
/// (AJ-E202 Req.3 guardrail; AJ-E208 extends with the full detection
/// matrix and the user-chosen redaction policy — this floor stays).
#[must_use]
pub fn credential_scan(bytes: &[u8]) -> Vec<CredentialHit> {
    let text = String::from_utf8_lossy(bytes);
    let mut hits = Vec::new();
    for line in text.lines() {
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY-----") {
            hits.push(CredentialHit {
                kind: "private-key",
            });
        } else if line.contains("\"sk-") || line.starts_with("sk-") {
            hits.push(CredentialHit { kind: "api-key" });
        } else if line.contains("Bearer ") && line.contains("eyJ") {
            hits.push(CredentialHit { kind: "bearer" });
        }
    }
    hits
}

/// Builds a manifest row for content already in memory.
#[must_use]
pub fn row_for(rel_path: &str, bytes: &[u8], kind: &str, disposition: &str) -> ManifestRow {
    ManifestRow {
        rel_path: rel_path.to_owned(),
        sha256: sha256_hex(bytes),
        bytes: bytes.len() as u64,
        kind: kind.to_owned(),
        disposition: disposition.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        // sha256("abc") — the standard test vector.
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn credential_guard_catches_keys_but_not_prose() {
        assert!(credential_scan(b"the model wrote normal text").is_empty());
        assert_eq!(
            credential_scan(b"-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n"),
            vec![CredentialHit {
                kind: "private-key"
            }]
        );
        assert_eq!(
            credential_scan(b"token = \"sk-abcdef123456\""),
            vec![CredentialHit { kind: "api-key" }]
        );
        assert!(!credential_scan(b"Authorization: Bearer eyJhbGciOi").is_empty());
    }
}

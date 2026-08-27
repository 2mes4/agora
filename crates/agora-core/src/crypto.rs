//! Cryptographic primitives for AGORA trust and privacy (M5).
//!
//! Provides:
//! - **Ed25519** digital signatures for envelope authenticity and non-repudiation.
//! - **X25519** + **ChaCha20-Poly1305** hybrid encryption for End-to-End Encrypted (E2EE) sealed envelopes.
//! - **AgentKeypair** bundle for agent identity and encryption key management.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use ed25519_dalek::ed25519::signature::SignerMut;
use ed25519_dalek::Verifier;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::envelope::Envelope;
use crate::error::CoreError;

/// An Ed25519 signing private key.
#[derive(Clone)]
pub struct SigningKey {
    inner: ed25519_dalek::SigningKey,
}

impl SigningKey {
    /// Generate a new random signing key using OS randomness.
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        Self {
            inner: ed25519_dalek::SigningKey::generate(&mut csprng),
        }
    }

    /// Construct a signing key from 32 raw bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            inner: ed25519_dalek::SigningKey::from_bytes(bytes),
        }
    }

    /// Export key as raw 32 bytes.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }

    /// Encode key as a hexadecimal string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Decode key from a hexadecimal string.
    pub fn from_hex(s: &str) -> Result<Self, CoreError> {
        let bytes = hex::decode(s).map_err(|e| CoreError::Crypto(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(CoreError::Crypto(
                "invalid key length for Ed25519 signing key".into(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(&arr))
    }

    /// Get the corresponding public verifying key.
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey {
            inner: self.inner.verifying_key(),
        }
    }

    /// Sign arbitrary message bytes.
    pub fn sign(&mut self, message: &[u8]) -> [u8; 64] {
        self.inner.sign(message).to_bytes()
    }
}

/// An Ed25519 public verifying key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyingKey {
    inner: ed25519_dalek::VerifyingKey,
}

impl VerifyingKey {
    /// Construct a verifying key from 32 raw bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CoreError> {
        let inner = ed25519_dalek::VerifyingKey::from_bytes(bytes)
            .map_err(|e| CoreError::Crypto(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Export key as raw 32 bytes.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }

    /// Encode key as a hexadecimal string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Decode key from a hexadecimal string.
    pub fn from_hex(s: &str) -> Result<Self, CoreError> {
        let bytes = hex::decode(s).map_err(|e| CoreError::Crypto(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(CoreError::Crypto(
                "invalid length for Ed25519 public key".into(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(&arr)
    }

    /// Verify an Ed25519 signature over message bytes.
    pub fn verify(&self, message: &[u8], signature_bytes: &[u8; 64]) -> Result<(), CoreError> {
        let sig = ed25519_dalek::Signature::from_bytes(signature_bytes);
        self.inner
            .verify(message, &sig)
            .map_err(|e| CoreError::Crypto(format!("signature verification failed: {e}")))
    }
}

/// An X25519 static private key for Diffie-Hellman key exchange.
pub struct EncryptionSecretKey {
    inner: x25519_dalek::StaticSecret,
}

impl EncryptionSecretKey {
    /// Generate a new random X25519 secret key.
    pub fn generate() -> Self {
        let csprng = OsRng;
        Self {
            inner: x25519_dalek::StaticSecret::random_from_rng(csprng),
        }
    }

    /// Construct from 32 raw bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            inner: x25519_dalek::StaticSecret::from(*bytes),
        }
    }

    /// Export key as raw 32 bytes.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }

    /// Encode key as a hexadecimal string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Decode key from a hexadecimal string.
    pub fn from_hex(s: &str) -> Result<Self, CoreError> {
        let bytes = hex::decode(s).map_err(|e| CoreError::Crypto(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(CoreError::Crypto(
                "invalid key length for X25519 secret key".into(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(&arr))
    }

    /// Get the corresponding public encryption key.
    pub fn public_key(&self) -> EncryptionPublicKey {
        EncryptionPublicKey {
            inner: x25519_dalek::PublicKey::from(&self.inner),
        }
    }

    /// Perform Diffie-Hellman key exchange with another public key to compute a 32-byte shared secret.
    pub fn diffie_hellman(&self, their_public: &EncryptionPublicKey) -> [u8; 32] {
        self.inner.diffie_hellman(&their_public.inner).to_bytes()
    }
}

/// An X25519 public encryption key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncryptionPublicKey {
    inner: x25519_dalek::PublicKey,
}

impl EncryptionPublicKey {
    /// Construct from 32 raw bytes.
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            inner: x25519_dalek::PublicKey::from(*bytes),
        }
    }

    /// Export key as raw 32 bytes.
    pub fn to_bytes(&self) -> [u8; 32] {
        *self.inner.as_bytes()
    }

    /// Encode key as a hexadecimal string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Decode key from a hexadecimal string.
    pub fn from_hex(s: &str) -> Result<Self, CoreError> {
        let bytes = hex::decode(s).map_err(|e| CoreError::Crypto(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(CoreError::Crypto(
                "invalid length for X25519 public key".into(),
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(&arr))
    }
}

/// Bundle containing an agent's signing and encryption key pairs.
pub struct AgentKeypair {
    pub signing_key: SigningKey,
    pub encryption_secret: EncryptionSecretKey,
}

impl AgentKeypair {
    /// Generate a fresh random keypair for an agent.
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(),
            encryption_secret: EncryptionSecretKey::generate(),
        }
    }

    /// The agent's Ed25519 public verifying key.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// The agent's X25519 public encryption key.
    pub fn encryption_public_key(&self) -> EncryptionPublicKey {
        self.encryption_secret.public_key()
    }
}

/// An End-to-End Encrypted (E2EE) sealed payload.
///
/// Encrypted with ChaCha20-Poly1305 using an ephemeral ECDH shared secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedPayload {
    /// Ephemeral X25519 public key of the sender for this message (hex encoded).
    pub ephemeral_public_key: String,
    /// 12-byte ChaCha20-Poly1305 nonce (hex encoded).
    pub nonce: String,
    /// Encrypted ciphertext with authentication tag (hex encoded).
    pub ciphertext: String,
}

/// Encrypt a payload for a recipient's X25519 public key using ChaCha20-Poly1305.
pub fn seal_payload(
    payload: &serde_json::Value,
    recipient_pubkey: &EncryptionPublicKey,
) -> Result<SealedPayload, CoreError> {
    let raw_bytes = serde_json::to_vec(payload)?;

    // 1. Generate ephemeral X25519 secret and public key
    let ephemeral_secret = EncryptionSecretKey::generate();
    let ephemeral_public = ephemeral_secret.public_key();

    // 2. Derive 32-byte shared secret
    let shared_secret = ephemeral_secret.diffie_hellman(recipient_pubkey);

    // 3. Generate 12-byte random nonce
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // 4. Encrypt with ChaCha20-Poly1305
    let cipher = ChaCha20Poly1305::new_from_slice(&shared_secret)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    let ciphertext = cipher
        .encrypt(nonce, raw_bytes.as_ref())
        .map_err(|e| CoreError::Crypto(format!("encryption failed: {e}")))?;

    Ok(SealedPayload {
        ephemeral_public_key: ephemeral_public.to_hex(),
        nonce: hex::encode(nonce_bytes),
        ciphertext: hex::encode(ciphertext),
    })
}

/// Decrypt a sealed payload using the recipient's X25519 secret key.
pub fn unseal_payload(
    sealed: &SealedPayload,
    recipient_secret: &EncryptionSecretKey,
) -> Result<serde_json::Value, CoreError> {
    let ephemeral_pubkey = EncryptionPublicKey::from_hex(&sealed.ephemeral_public_key)?;
    let nonce_bytes = hex::decode(&sealed.nonce).map_err(|e| CoreError::Crypto(e.to_string()))?;
    if nonce_bytes.len() != 12 {
        return Err(CoreError::Crypto(
            "invalid nonce length for sealed payload".into(),
        ));
    }
    let ciphertext =
        hex::decode(&sealed.ciphertext).map_err(|e| CoreError::Crypto(e.to_string()))?;

    // 1. Derive shared secret
    let shared_secret = recipient_secret.diffie_hellman(&ephemeral_pubkey);

    // 2. Decrypt with ChaCha20-Poly1305
    let cipher = ChaCha20Poly1305::new_from_slice(&shared_secret)
        .map_err(|e| CoreError::Crypto(e.to_string()))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| CoreError::Crypto(format!("decryption failed: {e}")))?;

    let value: serde_json::Value = serde_json::from_slice(&plaintext)?;
    Ok(value)
}

/// Compute the canonical byte representation of an envelope for signing.
pub fn canonical_signing_bytes(envelope: &Envelope) -> Result<Vec<u8>, CoreError> {
    let payload_str = serde_json::to_string(&envelope.payload)?;
    let sealed_str = match &envelope.sealed {
        Some(s) => serde_json::to_string(s)?,
        None => String::new(),
    };
    let canonical = format!(
        "AGORA-V1|sender:{}|target:{}|intent:{}|ttl:{}|nonce:{}|context:{}|payload:{}|sealed:{}",
        envelope.sender,
        envelope.target,
        envelope.intent,
        envelope.ttl_ms.map(|t| t.to_string()).unwrap_or_default(),
        envelope.nonce.as_deref().unwrap_or(""),
        envelope.context_uri.as_deref().unwrap_or(""),
        payload_str,
        sealed_str
    );
    Ok(canonical.into_bytes())
}

/// Sign an envelope with the sender's signing key.
/// Populates `envelope.nonce`, `envelope.signer_public_key`, and `envelope.signature`.
pub fn sign_envelope(envelope: &mut Envelope, key: &mut SigningKey) -> Result<(), CoreError> {
    if envelope.nonce.is_none() {
        let mut nonce_bytes = [0u8; 16];
        OsRng.fill_bytes(&mut nonce_bytes);
        envelope.nonce = Some(hex::encode(nonce_bytes));
    }
    envelope.signer_public_key = Some(key.verifying_key().to_hex());

    let bytes_to_sign = canonical_signing_bytes(envelope)?;
    let sig = key.sign(&bytes_to_sign);
    envelope.signature = Some(hex::encode(sig));
    Ok(())
}

/// Verify the digital signature on an envelope.
/// Returns `Ok(())` if signature is valid.
pub fn verify_envelope_signature(envelope: &Envelope) -> Result<(), CoreError> {
    let pub_key_hex = envelope
        .signer_public_key
        .as_ref()
        .ok_or_else(|| CoreError::Crypto("envelope has no signer_public_key".into()))?;
    let sig_hex = envelope
        .signature
        .as_ref()
        .ok_or_else(|| CoreError::Crypto("envelope has no signature".into()))?;

    let verifying_key = VerifyingKey::from_hex(pub_key_hex)?;
    let sig_bytes = hex::decode(sig_hex).map_err(|e| CoreError::Crypto(e.to_string()))?;
    if sig_bytes.len() != 64 {
        return Err(CoreError::Crypto(
            "invalid signature length (expected 64 bytes)".into(),
        ));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);

    let bytes_to_verify = canonical_signing_bytes(envelope)?;
    verifying_key.verify(&bytes_to_verify, &sig_arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ed25519_sign_and_verify_roundtrip() {
        let mut key = SigningKey::generate();
        let vk = key.verifying_key();
        let message = b"Hello, AGORA trust and privacy!";

        let sig = key.sign(message);
        assert!(vk.verify(message, &sig).is_ok());

        // Corrupted message must fail
        assert!(vk.verify(b"tampered message", &sig).is_err());
    }

    #[test]
    fn hex_encoding_keys_roundtrip() {
        let key = SigningKey::generate();
        let hex_priv = key.to_hex();
        let restored_priv = SigningKey::from_hex(&hex_priv).unwrap();
        assert_eq!(key.to_bytes(), restored_priv.to_bytes());

        let vk = key.verifying_key();
        let hex_pub = vk.to_hex();
        let restored_vk = VerifyingKey::from_hex(&hex_pub).unwrap();
        assert_eq!(vk, restored_vk);
    }

    #[test]
    fn envelope_signing_and_verification() {
        let mut key = SigningKey::generate();
        let mut env = Envelope::new(
            "alice",
            "bob",
            "greet",
            json!({ "message": "classified data" }),
        );

        sign_envelope(&mut env, &mut key).unwrap();
        assert!(env.signature.is_some());
        assert!(env.signer_public_key.is_some());
        assert!(env.nonce.is_some());

        // Verification must pass
        assert!(verify_envelope_signature(&env).is_ok());

        // Tampering with payload must cause verification failure
        let mut tampered = env.clone();
        tampered.payload = json!({ "message": "tampered data" });
        assert!(verify_envelope_signature(&tampered).is_err());

        // Tampering with sender must fail
        let mut tampered_sender = env.clone();
        tampered_sender.sender = "eve".into();
        assert!(verify_envelope_signature(&tampered_sender).is_err());
    }

    #[test]
    fn sealed_envelope_encryption_and_decryption() {
        let bob_secret = EncryptionSecretKey::generate();
        let bob_public = bob_secret.public_key();

        let original_payload = json!({
            "secret_report": "Agent operation successful",
            "token": 42
        });

        // Alice seals payload for Bob
        let sealed = seal_payload(&original_payload, &bob_public).unwrap();
        assert!(!sealed.ciphertext.is_empty());
        assert!(!sealed.ephemeral_public_key.is_empty());

        // Bob unseals payload
        let decrypted = unseal_payload(&sealed, &bob_secret).unwrap();
        assert_eq!(original_payload, decrypted);

        // Eve (with a different key) cannot unseal Bob's payload
        let eve_secret = EncryptionSecretKey::generate();
        assert!(unseal_payload(&sealed, &eve_secret).is_err());
    }
}

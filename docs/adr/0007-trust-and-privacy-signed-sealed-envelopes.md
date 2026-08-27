# ADR-0007: Trust & privacy: signed and sealed envelopes

Status: accepted

## Context

In an open, distributed agentic economy, agents communicate across untrusted or
multi-tenant infrastructure (gateways, message brokers, proxies). Without
cryptographic guarantees:
1. Payloads can be inspected or altered by intermediate nodes (loss of confidentiality
   and integrity).
2. Senders can be spoofed, preventing non-repudiation and reliable audit trails.
3. Message replay attacks could trigger duplicate agent actions and unauthorized costs.

The architecture must provide zero-trust privacy and cryptographic identity
without preventing gateways from routing messages, evaluating governance
policies, or billing based on metadata.

## Decision

AGORA implements a **dual-layer cryptographic envelope** model in `agora-core::crypto`:

1. **Digital Signatures (Ed25519)**:
   - Senders sign a canonical serialization of the envelope's routing headers and payload:
     `AGORA-V1|sender:<sender>|target:<target>|intent:<intent>|ttl:<ttl>|nonce:<nonce>|context:<uri>|payload:<json>|sealed:<sealed>`
   - The signature and the sender's public verifying key are attached to `Envelope.signature`
     and `Envelope.signer_public_key`.
   - Governance policies (`VerifySignature`) and target agents verify signatures deterministically.

2. **Replay Protection**:
   - Every signed envelope carries a unique random `nonce` and creation timestamp.
   - `ReplayProtection` policy enforces sliding-window uniqueness and drift limits (`max_drift_seconds`).

3. **End-to-End Encryption / Sealed Envelopes (X25519 + ChaCha20-Poly1305)**:
   - Agents advertise their X25519 public encryption key in their Agent Card (`encryption_key`).
   - When sealing a payload for a recipient, the sender performs an ephemeral ECDH exchange
     (`x25519-dalek`) to derive a 256-bit symmetric key, encrypting the payload using
     `ChaCha20-Poly1305` authenticated encryption.
   - The sealed envelope carries `ephemeral_public_key`, `nonce`, and `ciphertext`.
   - **Routing headers stay in plaintext**: `sender`, `target`, `intent`, `ttl_ms`, and `nonce`
     remain readable by gateways and the governance chain, enabling routing and policy enforcement
     without disclosing payload contents.

4. **Key Management**:
   - `AgentKeypair` bundles Ed25519 signing and X25519 encryption key pairs.
   - `agora keys generate` CLI command outputs agent keys in standard hex format.

## Consequences

- Intermediate gateways, message brokers (NATS), and audit logs cannot decrypt E2EE payloads.
- Identity and non-repudiation are cryptographically verifiable independently of transport auth.
- Policy enforcement (rate limits, routing, quotas) remains fully functional because routing metadata
  is preserved in the canonical envelope headers.
- Agents require key management infrastructure (key generation, storage, and advertisement in cards).

## References

- ADR-0001: Canonical envelope, multi-protocol core
- ADR-0003: Governance as a policy chain
- Roadmap Milestone 5: Trust & Privacy

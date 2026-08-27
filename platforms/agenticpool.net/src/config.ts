import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import * as crypto from 'node:crypto';
import { AgentCredentials } from './types.js';

export const DEFAULT_GATEWAY_URL = 'https://api.agenticpool.net';
export const DEFAULT_CONFIG_DIR = path.join(os.homedir(), '.agenticpool');
export const DEFAULT_CREDENTIALS_FILE = path.join(DEFAULT_CONFIG_DIR, 'credentials.json');

/**
 * Generate a new Ed25519 and X25519 keypair for an agent.
 */
export function generateAgentKeys(): {
  signingPublicKey: string;
  signingPrivateKey: string;
  encryptionPublicKey: string;
  encryptionPrivateKey: string;
} {
  const edKeyPair = crypto.generateKeyPairSync('ed25519', {
    publicKeyEncoding: { type: 'spki', format: 'der' },
    privateKeyEncoding: { type: 'pkcs8', format: 'der' },
  });

  const xKeyPair = crypto.generateKeyPairSync('x25519', {
    publicKeyEncoding: { type: 'spki', format: 'der' },
    privateKeyEncoding: { type: 'pkcs8', format: 'der' },
  });

  const edPubRaw = edKeyPair.publicKey.subarray(-32);
  const edPrivRaw = edKeyPair.privateKey.subarray(-32);
  const xPubRaw = xKeyPair.publicKey.subarray(-32);
  const xPrivRaw = xKeyPair.privateKey.subarray(-32);

  return {
    signingPublicKey: edPubRaw.toString('hex'),
    signingPrivateKey: edPrivRaw.toString('hex'),
    encryptionPublicKey: xPubRaw.toString('hex'),
    encryptionPrivateKey: xPrivRaw.toString('hex'),
  };
}

/**
 * Check if the agent credentials exist locally.
 */
export function isInitialized(filePath: string = DEFAULT_CREDENTIALS_FILE): boolean {
  return fs.existsSync(filePath);
}

/**
 * Load credentials from local disk.
 */
export function loadCredentials(filePath: string = DEFAULT_CREDENTIALS_FILE): AgentCredentials {
  if (!fs.existsSync(filePath)) {
    throw new Error(
      `AgenticPool credentials not found at '${filePath}'. Run 'agenticpool init' first.`
    );
  }

  try {
    const raw = fs.readFileSync(filePath, 'utf-8');
    return JSON.parse(raw) as AgentCredentials;
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    throw new Error(`Failed to parse credentials file at '${filePath}': ${msg}`);
  }
}

/**
 * Save credentials to local disk with strict 0600 file permissions.
 */
export function saveCredentials(
  credentials: AgentCredentials,
  filePath: string = DEFAULT_CREDENTIALS_FILE
): void {
  const dir = path.dirname(filePath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
  }

  const json = JSON.stringify(credentials, null, 2);
  fs.writeFileSync(filePath, json, { encoding: 'utf-8', mode: 0o600 });
}

/**
 * Delete local credentials.
 */
export function clearCredentials(filePath: string = DEFAULT_CREDENTIALS_FILE): void {
  if (fs.existsSync(filePath)) {
    fs.unlinkSync(filePath);
  }
}

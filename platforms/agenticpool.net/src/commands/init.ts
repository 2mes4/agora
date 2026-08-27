import * as crypto from 'node:crypto';
import { generateAgentKeys, saveCredentials, isInitialized, DEFAULT_GATEWAY_URL, DEFAULT_CREDENTIALS_FILE } from '../config.js';
import { DuckiesLedger, INITIAL_FAUCET_AMOUNT } from '../economy/duckies.js';
import { AgentCredentials } from '../types.js';

export async function handleInit(options: {
  name?: string;
  gateway?: string;
  force?: boolean;
}): Promise<void> {
  if (isInitialized() && !options.force) {
    console.log(`⚠️  AgenticPool is already initialized on this machine.`);
    console.log(`Use --force to overwrite local credentials at ${DEFAULT_CREDENTIALS_FILE}.`);
    return;
  }

  const agentName = options.name || `agent-${crypto.randomBytes(4).toString('hex')}`;
  const gatewayUrl = options.gateway || DEFAULT_GATEWAY_URL;
  const apiKey = `agp_${crypto.randomBytes(16).toString('hex')}`;
  const keys = generateAgentKeys();

  const credentials: AgentCredentials = {
    agentId: `ap_id_${crypto.randomBytes(8).toString('hex')}`,
    agentName,
    apiKey,
    signingPublicKey: keys.signingPublicKey,
    signingPrivateKey: keys.signingPrivateKey,
    encryptionPublicKey: keys.encryptionPublicKey,
    encryptionPrivateKey: keys.encryptionPrivateKey,
    gatewayUrl,
    registeredAt: new Date().toISOString(),
  };

  saveCredentials(credentials);

  // Grant initial starter Duckies
  const ledger = new DuckiesLedger();
  ledger.grantStarterDuckies(agentName, INITIAL_FAUCET_AMOUNT);

  console.log(`\n🎉 AgenticPool.net Account Initialized!`);
  console.log(`=========================================`);
  console.log(`🤖 Agent Name:        ${credentials.agentName}`);
  console.log(`🆔 Agent ID:          ${credentials.agentId}`);
  console.log(`🌐 Gateway API:       ${credentials.gatewayUrl}`);
  console.log(`🔑 Public Key:        ${credentials.signingPublicKey.slice(0, 16)}...`);
  console.log(`💰 Starter Duckies:    ${INITIAL_FAUCET_AMOUNT} DUCKIES`);
  console.log(`📁 Credentials saved: ~/.agenticpool/credentials.json\n`);
  console.log(`👉 Run 'agenticpool whoami' or 'agenticpool balance' to get started.\n`);
}

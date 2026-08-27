import { loadCredentials } from '../config.js';
import { DuckiesLedger } from '../economy/duckies.js';
import { AgenticPoolClient } from '../client.js';

export async function handleWhoami(): Promise<void> {
  const credentials = loadCredentials();
  const ledger = new DuckiesLedger();
  const balance = ledger.getBalance(credentials.agentName);
  const client = new AgenticPoolClient({ credentials });

  let onlineStatus = 'unknown';
  try {
    const statusResp = await client.getStatus(credentials.agentName);
    onlineStatus = statusResp?.isOnline ? '🟢 online' : '⚪ offline';
  } catch {
    onlineStatus = '⚪ disconnected';
  }

  console.log(`\n🤖 AgenticPool Agent Identity`);
  console.log(`=========================================`);
  console.log(`Name:             ${credentials.agentName}`);
  console.log(`ID:               ${credentials.agentId}`);
  console.log(`Gateway:          ${credentials.gatewayUrl}`);
  console.log(`Network Status:   ${onlineStatus}`);
  console.log(`Signing Key:      ${credentials.signingPublicKey}`);
  console.log(`Encryption Key:   ${credentials.encryptionPublicKey}`);
  console.log(`Registered At:    ${credentials.registeredAt}`);
  console.log(`\n💰 Duckies Balance:`);
  console.log(`  Available:      ${balance.available} DUCKIES`);
  console.log(`  In Escrow:      ${balance.lockedInEscrow} DUCKIES`);
  console.log(`  Total Earned:   ${balance.totalEarned} DUCKIES`);
  console.log(`  Total Spent:    ${balance.totalSpent} DUCKIES\n`);
}

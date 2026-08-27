import { loadCredentials } from '../config.js';
import { DuckiesLedger } from '../economy/duckies.js';

export async function handleBalance(options: { ledger?: boolean }): Promise<void> {
  const credentials = loadCredentials();
  const ledger = new DuckiesLedger();
  const balance = ledger.getBalance(credentials.agentName);

  console.log(`\n🦆 Duckies Balance for '${credentials.agentName}'`);
  console.log(`=========================================`);
  console.log(`💰 Available:        ${balance.available} DUCKIES`);
  console.log(`🔒 Locked in Escrow: ${balance.lockedInEscrow} DUCKIES`);
  console.log(`📈 Lifetime Earned:  ${balance.totalEarned} DUCKIES`);
  console.log(`📉 Lifetime Spent:   ${balance.totalSpent} DUCKIES\n`);

  if (options.ledger) {
    const txs = ledger.getTransactions(credentials.agentName);
    console.log(`📜 Transaction Ledger (${txs.length} entries):`);
    console.log(`-----------------------------------------`);
    if (txs.length === 0) {
      console.log(`  No transactions recorded yet.`);
    } else {
      for (const tx of txs.slice(-10).reverse()) {
        const sign = tx.toAgent === credentials.agentName ? '+' : '-';
        console.log(
          `  [${new Date(tx.timestamp).toLocaleString()}] ${tx.type.padEnd(14)} ${sign}${tx.amount} DUCKIES  (${tx.description || ''})`
        );
      }
    }
    console.log('');
  }
}

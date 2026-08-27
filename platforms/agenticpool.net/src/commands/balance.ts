import { loadCredentials } from '../config.js';
import { DuckiesLedger } from '../economy/duckies.js';

export async function handleBalance(options: { ledger?: boolean }): Promise<void> {
  const credentials = loadCredentials();
  const ledger = new DuckiesLedger();
  const balance = ledger.getBalance(credentials.agentName);
  const reputation = ledger.getReputation(credentials.agentName);

  console.log(`\n🦆 Duckies Wallet for '${credentials.agentName}'`);
  console.log(`=========================================`);
  console.log(`💰 Total Available:     ${balance.available} DUCKIES`);
  console.log(`  ├─ 🎫 Faucet Vouchers: ${balance.availableVoucher} DUCKIES (consumption-only)`);
  console.log(`  └─ 💵 Earned Duckies:  ${balance.availableEarned} DUCKIES (from favors)`);
  console.log(`🔒 Locked in Escrow:    ${balance.lockedInEscrow} DUCKIES`);
  console.log(`📈 Lifetime Earned:     ${balance.totalEarned} DUCKIES`);
  console.log(`📉 Lifetime Spent:      ${balance.totalSpent} DUCKIES`);
  console.log(`🔥 Network Fees Burned: ${balance.totalBurned} DUCKIES`);
  console.log(`⭐ Trust Tier:          ${reputation.trustTier.toUpperCase()} (Score: ${reputation.score}/100)\n`);

  if (options.ledger) {
    const txs = ledger.getTransactions(credentials.agentName);
    console.log(`📜 Transaction Ledger (${txs.length} entries):`);
    console.log(`-----------------------------------------`);
    if (txs.length === 0) {
      console.log(`  No transactions recorded yet.`);
    } else {
      for (const tx of txs.slice(-15).reverse()) {
        const sign = tx.toAgent === credentials.agentName ? '+' : '-';
        console.log(
          `  [${new Date(tx.timestamp).toLocaleString()}] ${tx.type.padEnd(16)} ${sign}${tx.amount.toFixed(2)} DUCKIES  (${tx.description || ''})`
        );
      }
    }
    console.log('');
  }
}

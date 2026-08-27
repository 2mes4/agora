import { loadCredentials } from '../config.js';
import { DuckiesLedger } from '../economy/duckies.js';

export async function handleReputation(targetAgentName?: string): Promise<void> {
  const credentials = loadCredentials();
  const agentName = targetAgentName || credentials.agentName;
  const ledger = new DuckiesLedger();
  const rep = ledger.getReputation(agentName);
  const disputes = ledger.getDisputes(agentName);

  let badge = '⚪';
  if (rep.trustTier === 'gold') badge = '🥇';
  else if (rep.trustTier === 'silver') badge = '🥈';
  else if (rep.trustTier === 'bronze') badge = '🥉';

  console.log(`\n${badge} Reputation & Trust Profile: '${agentName}'`);
  console.log(`=========================================`);
  console.log(`⭐ Trust Tier:       ${badge} ${rep.trustTier.toUpperCase()}`);
  console.log(`📊 Reputation Score: ${rep.score} / 100`);
  console.log(`🎯 Completion Rate:  ${(rep.completionRate * 100).toFixed(1)}%`);
  console.log(`✅ Completed Favors: ${rep.completedFavors}`);
  console.log(`⚠️  Disputed Favors:  ${rep.disputedFavors}`);
  console.log(`🚫 Canceled Favors:  ${rep.canceledFavors}`);
  console.log(`💰 Total Volume:     ${rep.totalVolumeDuckies.toFixed(2)} DUCKIES\n`);

  if (disputes.length > 0) {
    console.log(`⚖️ Dispute History (${disputes.length} records):`);
    console.log(`-----------------------------------------`);
    for (const d of disputes) {
      console.log(`• [${d.id}] Status: ${d.status}`);
      console.log(`  Requester: ${d.fromAgent} ──> Worker: ${d.targetAgent}`);
      console.log(`  Amount:    ${d.amount} DUCKIES`);
      console.log(`  Reason:    ${d.reason}`);
      console.log(`-----------------------------------------`);
    }
    console.log('');
  }
}

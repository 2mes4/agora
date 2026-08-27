import { loadCredentials } from '../config.js';
import { AgenticPoolClient } from '../client.js';
import { DuckiesLedger } from '../economy/duckies.js';

export async function handleRequestFavor(options: {
  target: string;
  service: string;
  message: string;
  price?: number;
}): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });
  const ledger = new DuckiesLedger();

  const priceDuckies = options.price ?? 5.0;

  console.log(`\n🤝 Requesting Agentic Favor...`);
  console.log(`=========================================`);
  console.log(`🎯 Target Agent:   ${options.target}`);
  console.log(`🆔 Service ID:     ${options.service}`);
  console.log(`💰 Agreed Price:   ${priceDuckies} DUCKIES`);
  console.log(`💬 Favor Request:  "${options.message}"`);

  // 1. Lock Duckies in Escrow
  try {
    ledger.lockEscrow(
      credentials.agentName,
      options.target,
      priceDuckies,
      options.service
    );
    console.log(`🔒 Locked ${priceDuckies} DUCKIES in local escrow.`);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`\n❌ Escrow lock failed: ${msg}`);
    return;
  }

  // 2. Delegate Favor to Target Agent
  try {
    console.log(`📡 Sending task to agent '${options.target}'...`);
    const resp = await client.requestFavor(options.target, options.message);

    const task = resp?.result || resp;
    const taskState = task?.status?.state || 'completed';
    const reply = task?.status?.message?.parts?.[0]?.text;

    // 3. Anti-Fraud Output Validation
    const validation = ledger.validateOutput(reply);

    if (taskState === 'completed' && validation.valid) {
      // Settle Escrow (with 3% burn fee)
      const { payment, burn, payout } = ledger.settleEscrow(
        credentials.agentName,
        options.target,
        priceDuckies,
        options.service,
        task?.id
      );

      console.log(`\n🎉 Favor Completed Successfully!`);
      console.log(`💸 ${payment.amount} DUCKIES settled:`);
      console.log(`  ├─ Worker received: ${payout.amount} DUCKIES`);
      console.log(`  └─ Network burn fee: ${burn.amount} DUCKIES (3% anti-wash fee burned)`);

      console.log(`\n📬 Result from '${options.target}':`);
      console.log(`-----------------------------------------`);
      console.log(reply);
      console.log(`-----------------------------------------\n`);
    } else {
      const reason = !validation.valid ? validation.reason! : `Task state was '${taskState}'`;
      // Open Dispute / Refund Escrow
      ledger.openDispute(
        credentials.agentName,
        options.target,
        priceDuckies,
        options.service,
        reason,
        task?.id
      );
      ledger.refundEscrow(
        credentials.agentName,
        options.target,
        priceDuckies,
        options.service,
        reason
      );
      console.log(`\n⚠️  Favor rejected by anti-fraud check (${reason}).`);
      console.log(`🛡️ Escrow refunded to '${credentials.agentName}'.`);
    }
  } catch (err: unknown) {
    ledger.refundEscrow(
      credentials.agentName,
      options.target,
      priceDuckies,
      options.service,
      'Network or execution error'
    );
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`\n❌ Delegation error: ${msg}. Escrow refunded.`);
  }
}

export async function handleDisputeFavor(options: {
  target: string;
  service: string;
  amount: number;
  reason: string;
  taskId?: string;
}): Promise<void> {
  const credentials = loadCredentials();
  const ledger = new DuckiesLedger();

  const dispute = ledger.openDispute(
    credentials.agentName,
    options.target,
    options.amount,
    options.service,
    options.reason,
    options.taskId
  );

  console.log(`\n⚖️ Dispute Opened!`);
  console.log(`=========================================`);
  console.log(`🆔 Dispute ID:    ${dispute.id}`);
  console.log(`🎯 Target Agent:  ${options.target}`);
  console.log(`💰 Amount:        ${options.amount} DUCKIES`);
  console.log(`📝 Reason:        ${options.reason}`);
  console.log(`⏳ Status:        ${dispute.status}`);
}

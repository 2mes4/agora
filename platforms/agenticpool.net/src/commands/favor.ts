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

    if (taskState === 'completed') {
      // 3. Settle Escrow
      ledger.settleEscrow(
        credentials.agentName,
        options.target,
        priceDuckies,
        options.service,
        task?.id
      );
      console.log(`\n🎉 Favor Completed Successfully!`);
      console.log(`💸 ${priceDuckies} DUCKIES transferred to '${options.target}'.`);

      const reply = task?.status?.message?.parts?.[0]?.text || JSON.stringify(task);
      console.log(`\n📬 Result from '${options.target}':`);
      console.log(`-----------------------------------------`);
      console.log(reply);
      console.log(`-----------------------------------------\n`);
    } else {
      // Refund Escrow
      ledger.refundEscrow(
        credentials.agentName,
        options.target,
        priceDuckies,
        options.service,
        `Task ended in state '${taskState}'`
      );
      console.log(`\n⚠️  Favor did not complete (State: ${taskState}). Escrow refunded.`);
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

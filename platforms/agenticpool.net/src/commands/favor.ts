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

      // 4. Proof-of-Execution: Record +1 Duckie de Goma on Trust Graph via Task Review
      if (task?.id) {
        try {
          await client.reviewTask(task.id, {
            outcome: 'satisfied',
            worker: options.target,
            feedback: 'Favor delivered and validated',
          });
          console.log(`🦆 +1 Duckie de Goma awarded to '${options.target}' on the Trust Graph!`);
        } catch {
          // Non-blocking if gateway offline during local test
        }
      }
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

      if (task?.id) {
        try {
          await client.reviewTask(task.id, {
            outcome: 'fraud',
            worker: options.target,
            feedback: reason,
          });
          console.log(`🌑 Duckies de Plomo assessed to '${options.target}' on the Trust Graph.`);
        } catch {
          // Non-blocking
        }
      }
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
  const client = new AgenticPoolClient({ credentials });

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

  if (options.taskId) {
    try {
      await client.reviewTask(options.taskId, {
        outcome: 'disputed',
        worker: options.target,
        feedback: options.reason,
      });
      console.log(`⚖️ Task review recorded on Trust Graph for task '${options.taskId}'.`);
    } catch {
      // Non-blocking
    }
  }
}

export async function handleReviewTask(options: {
  taskId: string;
  worker: string;
  outcome: 'satisfied' | 'rejected' | 'disputed' | 'fraud';
  feedback?: string;
  recommender?: string;
}): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n📝 Submitting Task Review for '${options.taskId}'...`);
  try {
    const res = await client.reviewTask(options.taskId, {
      outcome: options.outcome,
      worker: options.worker,
      feedback: options.feedback,
      recommender: options.recommender,
    });

    console.log(`\n✅ Task Review Processed (Proof-of-Execution):`);
    console.log(`=========================================`);
    console.log(`🆔 Task ID:         ${res.taskId}`);
    console.log(`📊 Outcome:         ${res.outcome.toUpperCase()}`);
    console.log(`🦆 Goma Awarded:    +${res.gomaAwarded}`);
    console.log(`🌑 Plomo Assessed:  +${res.plomoAssessed}`);
    console.log(`👤 Evaluator:       ${res.edgeUpdated.fromAgent}`);
    console.log(`🎯 Worker:          ${res.edgeUpdated.toAgent}`);
    console.log(`📈 New Edge Goma:   🦆 ${res.edgeUpdated.goma} | 🌑 ${res.edgeUpdated.plomo.toFixed(1)}`);
    if (res.recommenderEdgeUpdated) {
      console.log(`⭐ Recommender (${options.recommender}) Edge Updated: 🦆 ${res.recommenderEdgeUpdated.recomGoma} Recom Goma`);
    }
    console.log(`=========================================\n`);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`❌ Failed to submit task review: ${msg}`);
  }
}

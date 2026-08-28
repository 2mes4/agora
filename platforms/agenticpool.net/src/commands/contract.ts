import { loadCredentials } from '../config.js';
import { AgenticPoolClient } from '../client.js';

export async function handleContractPropose(options: {
  worker: string;
  service: string;
  price: number;
  disputeCost?: number;
  prompt?: string;
  acceptancePrompt: string;
  recommender?: string;
}): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  const platformFee = Math.round(options.price * 0.03 * 100) / 100;
  const disputeCost = options.disputeCost ?? Math.max(0.5, Math.round(options.price * 0.18 * 100) / 100);

  console.log(`\n📜 Proposing Agentic Smart Contract...`);
  console.log(`======================================================`);
  console.log(`👤 Requester:          ${credentials.agentName}`);
  console.log(`🎯 Worker:             ${options.worker}`);
  console.log(`🆔 Service:            ${options.service}`);
  console.log(`💰 Price:              🪙 ${options.price} GDUCK`);
  console.log(`🏛️ Platform Fee (3%):  🪙 ${platformFee} GDUCK`);
  console.log(`⚖️ Dispute Fee (18%):  🪙 ${disputeCost} GDUCK (Loser-Pays, min 0.5 GDUCK)`);
  console.log(`🧪 Acceptance Prompt:  "${options.acceptancePrompt}"`);
  if (options.recommender) {
    console.log(`⭐ Recommender:        ${options.recommender}`);
  }

  const contract = await client.proposeContract({
    parties: {
      requester: credentials.agentName,
      worker: options.worker,
      recommender: options.recommender,
    },
    pricing: {
      servicePriceGduck: options.price,
      platformFeeGduck: platformFee,
      disputeCostGduck: disputeCost,
    },
    execution: {
      serviceId: options.service,
      timeoutSeconds: 300,
      inputPayload: { prompt: options.prompt || 'Execute contract task' },
      acceptanceCriteria: {
        prompt: options.acceptancePrompt,
      },
    },
    disputeTerms: {
      validationPrompt: `Arbitrate whether delivery conforms strictly to: "${options.acceptancePrompt}"`,
      loserPays: true,
      plomoPenalty: 2.0,
    },
  });

  console.log(`\n✅ Contract Created Successfully!`);
  console.log(`🆔 Contract ID:        ${contract.id}`);
  console.log(`📊 Status:             ${contract.status.toUpperCase()}`);
  console.log(`======================================================\n`);
}

export async function handleContractGet(id: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  const contract = await client.getContract(id);
  console.log(`\n======================================================`);
  console.log(`📜 Agentic Smart Contract: ${contract.id}`);
  console.log(`======================================================`);
  console.log(`📊 Status:             ${contract.status.toUpperCase()}`);
  console.log(`👤 Requester:          ${contract.parties.requester}`);
  console.log(`🎯 Worker:             ${contract.parties.worker}`);
  if (contract.parties.recommender) {
    console.log(`⭐ Recommender:        ${contract.parties.recommender}`);
  }
  console.log(`💰 Price:              🪙 ${contract.pricing.servicePriceGduck} GDUCK`);
  console.log(`⚖️ Dispute Cost:       🪙 ${contract.pricing.disputeCostGduck} GDUCK (Loser-Pays)`);
  console.log(`🧪 Acceptance Prompt:  "${contract.execution.acceptanceCriteria.prompt}"`);
  if (contract.outputPayload) {
    console.log(`📬 Delivered Output:   ${JSON.stringify(contract.outputPayload)}`);
  }
  if (contract.disputeTerms?.disputeReason) {
    console.log(`⚠️ Dispute Reason:     ${contract.disputeTerms.disputeReason}`);
  }
  console.log(`======================================================\n`);
}

export async function handleContractList(party?: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });
  const targetParty = party || credentials.agentName;

  const contracts = await client.listContracts(targetParty);
  console.log(`\n📋 Active Contracts for '${targetParty}': ${contracts.length}`);
  console.log(`------------------------------------------------------`);
  for (const c of contracts) {
    console.log(`• [${c.id}] ${c.parties.requester} ──> ${c.parties.worker} | 🪙 ${c.pricing.servicePriceGduck} GDUCK | Status: ${c.status}`);
  }
  console.log(`------------------------------------------------------\n`);
}

export async function handleContractAccept(id: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n✍️ Accepting Contract '${id}' as Worker '${credentials.agentName}'...`);
  const contract = await client.acceptContract(id, `sig_${credentials.agentName}_${Date.now()}`);
  console.log(`✅ Contract Accepted & Escrow Locked: ${contract.status.toUpperCase()}\n`);
}

export async function handleContractDeliver(id: string, outputJson: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  let parsedOutput: Record<string, any>;
  try {
    parsedOutput = JSON.parse(outputJson);
  } catch {
    parsedOutput = { result: outputJson };
  }

  console.log(`\n📬 Delivering Output for Contract '${id}'...`);
  const contract = await client.deliverContract(id, parsedOutput);
  console.log(`✅ Output Delivered: Status is now ${contract.status.toUpperCase()}\n`);
}

export async function handleContractEvaluate(id: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n🧪 Evaluating Acceptance Criteria for Contract '${id}'...`);
  const evalRes = await client.evaluateContractAcceptance(id);

  console.log(`\n======================================================`);
  console.log(`📊 Acceptance Criteria Evaluation`);
  console.log(`======================================================`);
  console.log(`🆔 Contract ID:    ${evalRes.contractId}`);
  console.log(`🎯 Prompt Result:  ${evalRes.result.toUpperCase()} (true / false / uncertain)`);
  console.log(`⭐ Quality Score:  ${evalRes.qualityScore}/100`);
  console.log(`📝 Rationale:      ${evalRes.rationale}`);
  console.log(`======================================================\n`);
}

export async function handleContractSettle(id: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n💸 Settling Contract '${id}' (Releasing Escrow in GDUCK)...`);
  const contract = await client.settleContract(id);
  console.log(`\n🎉 Contract Settled!`);
  console.log(`   • Worker (${contract.parties.worker}) received 🪙 ${contract.pricing.servicePriceGduck} GDUCK`);
  console.log(`   • Trust Graph updated: 🦆 +1 Duckie de Goma awarded!`);
  if (contract.parties.recommender) {
    console.log(`   • Recommender (${contract.parties.recommender}): 🦆 +0.5 Recom Goma awarded!`);
  }
  console.log(`\n`);
}

export async function handleContractDisconformity(id: string, notes: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n⚠️ Reporting Disconformity on Contract '${id}'...`);
  const contract = await client.reportDisconformity(id, notes);
  console.log(`📝 Disconformity Logged: Status is now ${contract.status.toUpperCase()}`);
  console.log(`   • Worker '${contract.parties.worker}' notified to inspect notes and submit a revised version.`);
  console.log(`   • Notes: "${notes}"\n`);
}

export async function handleContractDispute(id: string, reason: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n⚖️ Opening Dispute for Contract '${id}'...`);
  const contract = await client.disputeContract(id, reason);
  console.log(`⚠️ Dispute Registered! Status: ${contract.status.toUpperCase()}`);
  console.log(`   • Escrow frozen.`);
  console.log(`   • Awaiting counterparty acceptance to proceed to platform arbitration (Loser-Pays rule active).\n`);
}

export async function handleContractDisputeAccept(id: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n🤝 Accepting Dispute Arbitration for Contract '${id}' as '${credentials.agentName}'...`);
  const contract = await client.acceptDispute(id);
  console.log(`⚖️ Arbitration Accepted! Status: ${contract.status.toUpperCase()}`);
  console.log(`   • Both parties have agreed to arbitration.`);
  console.log(`   • Ready for platform arbitrator execution (dispute fee: 🪙 ${contract.pricing.disputeCostGduck} GDUCK to platform treasury).\n`);
}

export async function handleContractArbitrate(
  id: string,
  verdict: 'worker_wins' | 'requester_wins' | 'split',
  rationale: string
): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n⚖️ Arbitrating Disputed Contract '${id}' as '${credentials.agentName}'...`);
  const settlement = await client.arbitrateContract(id, {
    arbitrator: credentials.agentName,
    verdict,
    rationale,
  });

  console.log(`\n======================================================`);
  console.log(`🏛️ Arbitration Settlement (Loser-Pays Enforced)`);
  console.log(`======================================================`);
  console.log(`🆔 Contract ID:            ${settlement.contractId}`);
  console.log(`⚖️ Verdict:                ${settlement.verdict.toUpperCase()}`);
  console.log(`👨‍⚖️ Arbitrator:            ${settlement.arbitrator}`);
  console.log(`📝 Rationale:              ${settlement.rationale}`);
  console.log(`------------------------------------------------------`);
  console.log(`💸 Worker Payout:          🪙 ${settlement.workerPayoutGduck} GDUCK`);
  console.log(`🛡️ Requester Refund:       🪙 ${settlement.requesterRefundGduck} GDUCK`);
  console.log(`💥 Dispute Fee Paid By:    👤 ${settlement.disputeFeePaidBy} (🪙 ${settlement.disputeFeeAmountGduck} GDUCK)`);
  console.log(`🌑 Worker Plomo Delta:     +${settlement.workerPlomoDelta}`);
  console.log(`🌑 Requester Plomo Delta:  +${settlement.requesterPlomoDelta}`);
  if (settlement.recommenderPlomoDelta > 0) {
    console.log(`🌑 Recommender Slashed:    +${settlement.recommenderPlomoDelta} Plomo`);
  }
  console.log(`======================================================\n`);
}

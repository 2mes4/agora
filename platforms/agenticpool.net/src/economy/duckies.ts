import * as fs from 'node:fs';
import * as path from 'node:path';
import { DEFAULT_CONFIG_DIR } from '../config.js';
import {
  AgentReputation,
  DuckiesBalance,
  DuckiesTransaction,
  FavorDispute,
  TrustTier,
} from '../types.js';

export const INITIAL_FAUCET_AMOUNT = 100.0;
export const BURN_FEE_RATE = 0.03; // 3% burn fee on settlements to prevent wash-trading
export const DEFAULT_CANCELLATION_FEE_RATE = 0.10; // 10% compensation for in-progress compute

export const DEFAULT_LEDGER_FILE = path.join(DEFAULT_CONFIG_DIR, 'duckies_ledger.json');
export const DEFAULT_DISPUTES_FILE = path.join(DEFAULT_CONFIG_DIR, 'duckies_disputes.json');

export class DuckiesLedger {
  private transactions: DuckiesTransaction[] = [];
  private disputes: FavorDispute[] = [];
  private filePath: string;
  private disputesFilePath: string;

  constructor(
    filePath: string = DEFAULT_LEDGER_FILE,
    disputesFilePath: string = DEFAULT_DISPUTES_FILE
  ) {
    this.filePath = filePath;
    this.disputesFilePath = disputesFilePath;
    this.load();
  }

  private load(): void {
    if (fs.existsSync(this.filePath)) {
      try {
        const raw = fs.readFileSync(this.filePath, 'utf-8');
        this.transactions = JSON.parse(raw);
      } catch {
        this.transactions = [];
      }
    } else {
      this.transactions = [];
    }

    if (fs.existsSync(this.disputesFilePath)) {
      try {
        const raw = fs.readFileSync(this.disputesFilePath, 'utf-8');
        this.disputes = JSON.parse(raw);
      } catch {
        this.disputes = [];
      }
    } else {
      this.disputes = [];
    }
  }

  private save(): void {
    const dir = path.dirname(this.filePath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
    }
    fs.writeFileSync(this.filePath, JSON.stringify(this.transactions, null, 2), {
      encoding: 'utf-8',
      mode: 0o600,
    });
    fs.writeFileSync(this.disputesFilePath, JSON.stringify(this.disputes, null, 2), {
      encoding: 'utf-8',
      mode: 0o600,
    });
  }

  /**
   * Grant initial starter Duckies (locked as voucher credits).
   */
  grantStarterDuckies(agentName: string, amount: number = INITIAL_FAUCET_AMOUNT): void {
    if (this.transactions.some((t) => t.type === 'faucet' && t.toAgent === agentName)) {
      return; // already claimed
    }

    this.recordTransaction({
      id: `tx-faucet-${Date.now()}`,
      type: 'faucet',
      amount,
      fromAgent: 'agenticpool.faucet',
      toAgent: agentName,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: 'Starter Duckies voucher grant (consumption-only)',
    });
  }

  /**
   * Calculate current Duckies balance with voucher and earned separation.
   */
  getBalance(agentName: string): DuckiesBalance {
    let voucherBalance = 0;
    let earnedBalance = 0;
    let lockedInEscrow = 0;
    let totalEarned = 0;
    let totalSpent = 0;
    let totalBurned = 0;

    for (const tx of this.transactions) {
      if (tx.status !== 'completed' && tx.status !== 'pending') continue;

      if (tx.toAgent === agentName) {
        if (tx.type === 'faucet') {
          voucherBalance += tx.amount;
        } else if (tx.type === 'favor_payout' || tx.type === 'cancellation_fee') {
          earnedBalance += tx.amount;
          totalEarned += tx.amount;
        } else if (tx.type === 'escrow_refund') {
          earnedBalance += tx.amount;
          lockedInEscrow = Math.max(0, lockedInEscrow - tx.amount);
        }
      }

      if (tx.fromAgent === agentName) {
        if (tx.type === 'escrow_lock') {
          // Deduct from voucher first, then earned
          const deductFromVoucher = Math.min(voucherBalance, tx.amount);
          const deductFromEarned = tx.amount - deductFromVoucher;
          voucherBalance = Math.max(0, voucherBalance - deductFromVoucher);
          earnedBalance = Math.max(0, earnedBalance - deductFromEarned);
          lockedInEscrow += tx.amount;
        } else if (tx.type === 'favor_payment') {
          totalSpent += tx.amount;
          lockedInEscrow = Math.max(0, lockedInEscrow - tx.amount);
        } else if (tx.type === 'cancellation_fee') {
          totalSpent += tx.amount;
          lockedInEscrow = Math.max(0, lockedInEscrow - tx.amount);
        } else if (tx.type === 'burn_fee') {
          totalBurned += tx.amount;
        }
      }
    }

    const available = voucherBalance + earnedBalance;

    return {
      available: Math.max(0, available),
      availableVoucher: Math.max(0, voucherBalance),
      availableEarned: Math.max(0, earnedBalance),
      lockedInEscrow: Math.max(0, lockedInEscrow),
      totalEarned,
      totalSpent,
      totalBurned,
    };
  }

  /**
   * Lock Duckies in escrow before delegating a favor.
   */
  lockEscrow(
    fromAgent: string,
    targetAgent: string,
    amount: number,
    serviceId: string,
    taskId?: string
  ): DuckiesTransaction {
    const balance = this.getBalance(fromAgent);
    if (balance.available < amount) {
      throw new Error(
        `Insufficient Duckies: available ${balance.available} DUCKIES, but favor requires ${amount} DUCKIES.`
      );
    }

    return this.recordTransaction({
      id: `tx-escrow-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      type: 'escrow_lock',
      amount,
      fromAgent,
      toAgent: targetAgent,
      serviceId,
      taskId,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: `Locked ${amount} Duckies for service '${serviceId}' on agent '${targetAgent}'`,
    });
  }

  /**
   * Settle escrow payout upon successful favor completion, applying the 3% burn fee.
   */
  settleEscrow(
    fromAgent: string,
    targetAgent: string,
    amount: number,
    serviceId: string,
    taskId?: string
  ): { payment: DuckiesTransaction; burn: DuckiesTransaction; payout: DuckiesTransaction } {
    const burnFee = Math.round(amount * BURN_FEE_RATE * 100) / 100;
    const netPayout = Math.round((amount - burnFee) * 100) / 100;

    const payment = this.recordTransaction({
      id: `tx-pay-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      type: 'favor_payment',
      amount,
      fromAgent,
      toAgent: targetAgent,
      serviceId,
      taskId,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: `Favor payment: ${amount} Duckies for service '${serviceId}'`,
    });

    const burn = this.recordTransaction({
      id: `tx-burn-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      type: 'burn_fee',
      amount: burnFee,
      fromAgent,
      toAgent: 'agenticpool.burn',
      serviceId,
      taskId,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: `Network burn fee (3%): ${burnFee} Duckies burned to protect against wash-trading`,
    });

    const payout = this.recordTransaction({
      id: `tx-payout-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      type: 'favor_payout',
      amount: netPayout,
      fromAgent,
      toAgent: targetAgent,
      serviceId,
      taskId,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: `Favor payout: ${netPayout} Duckies net received from '${fromAgent}' (after ${burnFee} burn fee)`,
    });

    return { payment, burn, payout };
  }

  /**
   * Refund escrow if favor fails or is canceled before work began.
   */
  refundEscrow(
    fromAgent: string,
    targetAgent: string,
    amount: number,
    serviceId: string,
    reason: string
  ): DuckiesTransaction {
    return this.recordTransaction({
      id: `tx-refund-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      type: 'escrow_refund',
      amount,
      fromAgent: targetAgent,
      toAgent: fromAgent,
      serviceId,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: `Escrow refund: ${amount} Duckies returned to '${fromAgent}' (${reason})`,
    });
  }

  /**
   * Cancel in-progress favor with computational compensation for worker.
   */
  cancelEscrowWithCompensation(
    fromAgent: string,
    targetAgent: string,
    amount: number,
    serviceId: string,
    taskId?: string,
    compensationRate: number = DEFAULT_CANCELLATION_FEE_RATE
  ): { compensation: DuckiesTransaction; refund: DuckiesTransaction } {
    const compensationAmount = Math.round(amount * compensationRate * 100) / 100;
    const refundAmount = Math.round((amount - compensationAmount) * 100) / 100;

    const compensation = this.recordTransaction({
      id: `tx-comp-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      type: 'cancellation_fee',
      amount: compensationAmount,
      fromAgent,
      toAgent: targetAgent,
      serviceId,
      taskId,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: `Cancellation compensation: ${compensationAmount} Duckies awarded to worker '${targetAgent}' for in-progress compute`,
    });

    const refund = this.recordTransaction({
      id: `tx-refund-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      type: 'escrow_refund',
      amount: refundAmount,
      fromAgent: targetAgent,
      toAgent: fromAgent,
      serviceId,
      taskId,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: `Escrow refund after cancellation: ${refundAmount} Duckies returned to '${fromAgent}'`,
    });

    return { compensation, refund };
  }

  /**
   * Open a dispute on an unsatisfactory favor delivery.
   */
  openDispute(
    fromAgent: string,
    targetAgent: string,
    amount: number,
    serviceId: string,
    reason: string,
    taskId?: string
  ): FavorDispute {
    const dispute: FavorDispute = {
      id: `disp-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      fromAgent,
      targetAgent,
      amount,
      serviceId,
      taskId,
      reason,
      openedAt: new Date().toISOString(),
      status: 'open',
    };

    this.disputes.push(dispute);
    this.recordTransaction({
      id: `tx-disp-${Date.now()}`,
      type: 'dispute_opened',
      amount,
      fromAgent,
      toAgent: targetAgent,
      serviceId,
      taskId,
      timestamp: new Date().toISOString(),
      status: 'pending',
      description: `Dispute opened on favor: ${reason}`,
    });

    this.save();
    return dispute;
  }

  /**
   * Resolve dispute (full refund, full payout, or split).
   */
  resolveDispute(
    disputeId: string,
    outcome: 'refund' | 'payout' | 'split',
    splitRatio: number = 0.5
  ): FavorDispute {
    const dispute = this.disputes.find((d) => d.id === disputeId);
    if (!dispute) {
      throw new Error(`Dispute '${disputeId}' not found.`);
    }

    if (outcome === 'refund') {
      dispute.status = 'resolved_refund';
      this.refundEscrow(
        dispute.fromAgent,
        dispute.targetAgent,
        dispute.amount,
        dispute.serviceId,
        'Dispute resolved in favor of requester'
      );
    } else if (outcome === 'payout') {
      dispute.status = 'resolved_payout';
      this.settleEscrow(
        dispute.fromAgent,
        dispute.targetAgent,
        dispute.amount,
        dispute.serviceId,
        dispute.taskId
      );
    } else {
      dispute.status = 'resolved_split';
      const refundAmt = Math.round(dispute.amount * splitRatio * 100) / 100;
      const payoutAmt = Math.round((dispute.amount - refundAmt) * 100) / 100;
      this.refundEscrow(
        dispute.fromAgent,
        dispute.targetAgent,
        refundAmt,
        dispute.serviceId,
        'Dispute split refund'
      );
      this.settleEscrow(
        dispute.fromAgent,
        dispute.targetAgent,
        payoutAmt,
        dispute.serviceId,
        dispute.taskId
      );
    }

    dispute.resolvedAt = new Date().toISOString();
    this.save();
    return dispute;
  }

  /**
   * Validate that task output is non-empty and non-fraudulent.
   */
  validateOutput(output: unknown): { valid: boolean; reason?: string } {
    if (output === null || output === undefined) {
      return { valid: false, reason: 'Task output is empty or null.' };
    }

    if (typeof output === 'string') {
      if (output.trim().length === 0) {
        return { valid: false, reason: 'Task output contains empty string.' };
      }
      if (output.toLowerCase().startsWith('error:') || output.toLowerCase().includes('failed to execute')) {
        return { valid: false, reason: `Task output reported execution failure: ${output}` };
      }
    }

    return { valid: true };
  }

  /**
   * Calculate reputation and trust tier for an agent.
   */
  getReputation(agentName: string): AgentReputation {
    const fulfilled = this.transactions.filter(
      (tx) => tx.toAgent === agentName && tx.type === 'favor_payout'
    );
    const disputesAgainst = this.disputes.filter(
      (d) => d.targetAgent === agentName && d.status === 'resolved_refund'
    );
    const canceledAgainst = this.transactions.filter(
      (tx) => tx.toAgent === agentName && tx.type === 'cancellation_fee'
    );

    const completedFavors = fulfilled.length;
    const disputedFavors = disputesAgainst.length;
    const canceledFavors = canceledAgainst.length;
    const totalAttempted = completedFavors + disputedFavors;

    const completionRate =
      totalAttempted > 0
        ? Math.round((completedFavors / totalAttempted) * 100) / 100
        : 1.0;

    let totalVolumeDuckies = 0;
    for (const tx of fulfilled) {
      totalVolumeDuckies += tx.amount;
    }

    let trustTier: TrustTier = 'unverified';
    if (completedFavors >= 50 && completionRate >= 0.95) {
      trustTier = 'gold';
    } else if (completedFavors >= 20 && completionRate >= 0.90) {
      trustTier = 'silver';
    } else if (completedFavors >= 5 && completionRate >= 0.80) {
      trustTier = 'bronze';
    }

    // Base score 0 to 100
    const score = Math.min(
      100,
      Math.round(
        (completionRate * 70 +
          Math.min(30, completedFavors * 0.6) -
          disputedFavors * 15) *
          10
      ) / 10
    );

    return {
      agentName,
      score: Math.max(0, score),
      completedFavors,
      disputedFavors,
      canceledFavors,
      completionRate,
      trustTier,
      totalVolumeDuckies,
    };
  }

  /**
   * Get all transactions for an agent.
   */
  getTransactions(agentName: string): DuckiesTransaction[] {
    return this.transactions.filter(
      (tx) => tx.fromAgent === agentName || tx.toAgent === agentName
    );
  }

  /**
   * Get all disputes for an agent.
   */
  getDisputes(agentName: string): FavorDispute[] {
    return this.disputes.filter(
      (d) => d.fromAgent === agentName || d.targetAgent === agentName
    );
  }

  private recordTransaction(tx: DuckiesTransaction): DuckiesTransaction {
    this.transactions.push(tx);
    this.save();
    return tx;
  }
}

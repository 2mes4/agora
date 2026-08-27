import * as fs from 'node:fs';
import * as path from 'node:path';
import { DEFAULT_CONFIG_DIR } from '../config.js';
import { DuckiesBalance, DuckiesTransaction } from '../types.js';

export const INITIAL_FAUCET_AMOUNT = 100.0;
export const DEFAULT_LEDGER_FILE = path.join(DEFAULT_CONFIG_DIR, 'duckies_ledger.json');

export class DuckiesLedger {
  private transactions: DuckiesTransaction[] = [];
  private filePath: string;

  constructor(filePath: string = DEFAULT_LEDGER_FILE) {
    this.filePath = filePath;
    this.load();
  }

  /**
   * Load local ledger transactions.
   */
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
  }

  /**
   * Save ledger transactions.
   */
  private save(): void {
    const dir = path.dirname(this.filePath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true, mode: 0o700 });
    }
    fs.writeFileSync(this.filePath, JSON.stringify(this.transactions, null, 2), {
      encoding: 'utf-8',
      mode: 0o600,
    });
  }

  /**
   * Initialize agent wallet with starter Duckies.
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
      description: 'Starter Duckies grant for new agent account',
    });
  }

  /**
   * Calculate current Duckies balance for an agent.
   */
  getBalance(agentName: string): DuckiesBalance {
    let available = 0;
    let lockedInEscrow = 0;
    let totalEarned = 0;
    let totalSpent = 0;

    for (const tx of this.transactions) {
      if (tx.status !== 'completed' && tx.status !== 'pending') continue;

      if (tx.toAgent === agentName) {
        if (tx.type === 'faucet' || tx.type === 'favor_payout') {
          available += tx.amount;
          totalEarned += tx.amount;
        } else if (tx.type === 'escrow_refund') {
          available += tx.amount;
          lockedInEscrow = Math.max(0, lockedInEscrow - tx.amount);
        }
      }

      if (tx.fromAgent === agentName) {
        if (tx.type === 'escrow_lock') {
          available -= tx.amount;
          lockedInEscrow += tx.amount;
        } else if (tx.type === 'favor_payment') {
          totalSpent += tx.amount;
          lockedInEscrow = Math.max(0, lockedInEscrow - tx.amount);
        }
      }
    }

    return {
      available: Math.max(0, available),
      lockedInEscrow: Math.max(0, lockedInEscrow),
      totalEarned,
      totalSpent,
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
   * Settle escrow payout upon successful favor completion.
   */
  settleEscrow(
    fromAgent: string,
    targetAgent: string,
    amount: number,
    serviceId: string,
    taskId?: string
  ): { payment: DuckiesTransaction; payout: DuckiesTransaction } {
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
      description: `Favor settlement: ${amount} Duckies paid for service '${serviceId}'`,
    });

    const payout = this.recordTransaction({
      id: `tx-out-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      type: 'favor_payout',
      amount,
      fromAgent,
      toAgent: targetAgent,
      serviceId,
      taskId,
      timestamp: new Date().toISOString(),
      status: 'completed',
      description: `Favor payout: ${amount} Duckies received from '${fromAgent}' for service '${serviceId}'`,
    });

    return { payment, payout };
  }

  /**
   * Refund escrow if favor fails or is canceled.
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
   * Get all transactions related to an agent.
   */
  getTransactions(agentName: string): DuckiesTransaction[] {
    return this.transactions.filter(
      (tx) => tx.fromAgent === agentName || tx.toAgent === agentName
    );
  }

  /**
   * Record a new transaction.
   */
  private recordTransaction(tx: DuckiesTransaction): DuckiesTransaction {
    this.transactions.push(tx);
    this.save();
    return tx;
  }
}

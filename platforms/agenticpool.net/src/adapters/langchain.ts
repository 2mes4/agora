/**
 * LangChain & LangGraph Adapter for AgenticPool.net.
 */

import { AgenticPoolClient } from '../client.js';
import { DuckiesLedger } from '../economy/duckies.js';
import { loadCredentials } from '../config.js';

export interface LangChainToolConfig {
  targetAgent: string;
  serviceId: string;
  name: string;
  description: string;
  priceDuckies?: number;
}

/**
 * Standard LangChain tool wrapper that routes tool execution
 * to a remote AgenticPool agent and settles payment in Duckies.
 */
export class AgenticPoolLangChainTool {
  name: string;
  description: string;
  targetAgent: string;
  serviceId: string;
  priceDuckies: number;

  private client: AgenticPoolClient;
  private ledger: DuckiesLedger;
  private agentName: string;

  constructor(config: LangChainToolConfig) {
    this.name = config.name;
    this.description = config.description;
    this.targetAgent = config.targetAgent;
    this.serviceId = config.serviceId;
    this.priceDuckies = config.priceDuckies || 5.0;

    const credentials = loadCredentials();
    this.agentName = credentials.agentName;
    this.client = new AgenticPoolClient({ credentials });
    this.ledger = new DuckiesLedger();
  }

  /**
   * Invoked by LangChain agent when executing a tool call.
   */
  async _call(input: string | Record<string, unknown>): Promise<string> {
    const message = typeof input === 'string' ? input : JSON.stringify(input);

    // 1. Lock Duckies in Escrow
    this.ledger.lockEscrow(
      this.agentName,
      this.targetAgent,
      this.priceDuckies,
      this.serviceId
    );

    try {
      // 2. Request Remote Favor
      const resp = await this.client.requestFavor(this.targetAgent, message);
      const task = resp?.result || resp;

      // 3. Settle Payment
      this.ledger.settleEscrow(
        this.agentName,
        this.targetAgent,
        this.priceDuckies,
        this.serviceId,
        task?.id
      );

      const resultText = task?.status?.message?.parts?.[0]?.text || JSON.stringify(task);
      return resultText;
    } catch (err: unknown) {
      this.ledger.refundEscrow(
        this.agentName,
        this.targetAgent,
        this.priceDuckies,
        this.serviceId,
        'Tool execution error'
      );
      throw err;
    }
  }
}

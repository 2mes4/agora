/**
 * LlamaIndex ToolSpec Adapter for AgenticPool.net.
 */

import { AgenticPoolClient } from '../client.js';
import { DuckiesLedger } from '../economy/duckies.js';
import { loadCredentials } from '../config.js';

export interface LlamaIndexToolConfig {
  targetAgent: string;
  serviceId: string;
  name: string;
  description: string;
  priceDuckies?: number;
}

/**
 * LlamaIndex ToolSpec wrapper for executing remote agent queries and RAG favors.
 */
export class AgenticPoolLlamaIndexTool {
  name: string;
  description: string;
  targetAgent: string;
  serviceId: string;
  priceDuckies: number;

  private client: AgenticPoolClient;
  private ledger: DuckiesLedger;
  private agentName: string;

  constructor(config: LlamaIndexToolConfig) {
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
   * Execute query as a LlamaIndex tool.
   */
  async query(queryString: string): Promise<string> {
    this.ledger.lockEscrow(
      this.agentName,
      this.targetAgent,
      this.priceDuckies,
      this.serviceId
    );

    try {
      const resp = await this.client.requestFavor(this.targetAgent, queryString);
      const task = resp?.result || resp;

      this.ledger.settleEscrow(
        this.agentName,
        this.targetAgent,
        this.priceDuckies,
        this.serviceId,
        task?.id
      );

      return task?.status?.message?.parts?.[0]?.text || JSON.stringify(task);
    } catch (err: unknown) {
      this.ledger.refundEscrow(
        this.agentName,
        this.targetAgent,
        this.priceDuckies,
        this.serviceId,
        'LlamaIndex query error'
      );
      throw err;
    }
  }
}

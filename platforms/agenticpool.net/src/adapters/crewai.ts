/**
 * CrewAI Adapter for AgenticPool.net.
 */

import { AgenticPoolClient } from '../client.js';
import { DuckiesLedger } from '../economy/duckies.js';
import { loadCredentials } from '../config.js';

export interface CrewAiToolConfig {
  name: string;
  description: string;
  targetAgent: string;
  serviceId: string;
  priceDuckies?: number;
}

/**
 * CrewAI Tool wrapper allowing autonomous Crews to delegate specialized
 * subtasks to the global AgenticPool network.
 */
export class AgenticPoolCrewTool {
  name: string;
  description: string;
  targetAgent: string;
  serviceId: string;
  priceDuckies: number;

  private client: AgenticPoolClient;
  private ledger: DuckiesLedger;
  private agentName: string;

  constructor(config: CrewAiToolConfig) {
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
   * Run tool execution on behalf of a CrewAI agent.
   */
  async run(taskDescription: string): Promise<string> {
    this.ledger.lockEscrow(
      this.agentName,
      this.targetAgent,
      this.priceDuckies,
      this.serviceId
    );

    try {
      const resp = await this.client.requestFavor(this.targetAgent, taskDescription);
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
        'CrewAI tool execution failed'
      );
      throw err;
    }
  }
}

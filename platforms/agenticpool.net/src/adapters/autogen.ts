/**
 * Microsoft AutoGen Adapter for AgenticPool.net.
 */

import { AgenticPoolClient } from '../client.js';
import { DuckiesLedger } from '../economy/duckies.js';
import { loadCredentials } from '../config.js';

export interface AutoGenAgentConfig {
  name: string;
  targetAgent: string;
  serviceId?: string;
  priceDuckies?: number;
  systemMessage?: string;
}

/**
 * An AutoGen ConversableAgent proxy that routes messages to a remote
 * agent on AgenticPool.net and settles transactions in Duckies.
 */
export class AgenticPoolAutoGenAgent {
  name: string;
  targetAgent: string;
  serviceId: string;
  priceDuckies: number;
  systemMessage?: string;

  private client: AgenticPoolClient;
  private ledger: DuckiesLedger;
  private localAgentName: string;

  constructor(config: AutoGenAgentConfig) {
    this.name = config.name;
    this.targetAgent = config.targetAgent;
    this.serviceId = config.serviceId || 'generic.favor';
    this.priceDuckies = config.priceDuckies || 5.0;
    this.systemMessage = config.systemMessage;

    const credentials = loadCredentials();
    this.localAgentName = credentials.agentName;
    this.client = new AgenticPoolClient({ credentials });
    this.ledger = new DuckiesLedger();
  }

  /**
   * Generate reply in an AutoGen multi-agent group chat.
   */
  async generateReply(messages: Array<{ role: string; content: string }>): Promise<string> {
    const latestMessage = messages[messages.length - 1]?.content || '';

    this.ledger.lockEscrow(
      this.localAgentName,
      this.targetAgent,
      this.priceDuckies,
      this.serviceId
    );

    try {
      const resp = await this.client.requestFavor(this.targetAgent, latestMessage);
      const task = resp?.result || resp;

      this.ledger.settleEscrow(
        this.localAgentName,
        this.targetAgent,
        this.priceDuckies,
        this.serviceId,
        task?.id
      );

      return task?.status?.message?.parts?.[0]?.text || JSON.stringify(task);
    } catch (err: unknown) {
      this.ledger.refundEscrow(
        this.localAgentName,
        this.targetAgent,
        this.priceDuckies,
        this.serviceId,
        'AutoGen reply generation error'
      );
      throw err;
    }
  }
}

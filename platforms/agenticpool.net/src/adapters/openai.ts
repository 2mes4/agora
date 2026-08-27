/**
 * OpenAI Function Calling / Assistant API Adapter for AgenticPool.net.
 */

import { AgenticPoolClient } from '../client.js';
import { DuckiesLedger } from '../economy/duckies.js';
import { loadCredentials } from '../config.js';
import { PublishedService } from '../types.js';

export interface OpenAiFunctionDefinition {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
  };
}

/**
 * Convert an AgenticPool service into an OpenAI function definition schema.
 */
export function toOpenAiFunction(
  targetAgent: string,
  service: PublishedService
): OpenAiFunctionDefinition {
  const sanitizedName = `agent_${targetAgent.replace(/[^a-zA-Z0-9_]/g, '_')}_${service.id.replace(/[^a-zA-Z0-9_]/g, '_')}`;

  return {
    type: 'function',
    function: {
      name: sanitizedName,
      description: `${service.description || service.name} (Provided by agent '${targetAgent}', Price: ${service.priceDuckies} DUCKIES)`,
      parameters: {
        type: 'object',
        properties: {
          prompt: {
            type: 'string',
            description: 'The task description or query for the remote agent',
          },
        },
        required: ['prompt'],
      },
    },
  };
}

/**
 * Execute an OpenAI function call by routing it to the AgenticPool network and settling Duckies.
 */
export async function executeOpenAiToolCall(
  targetAgent: string,
  serviceId: string,
  argumentsJson: string,
  priceDuckies: number = 5.0
): Promise<string> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });
  const ledger = new DuckiesLedger();

  const parsedArgs = JSON.parse(argumentsJson);
  const prompt = parsedArgs.prompt || argumentsJson;

  ledger.lockEscrow(credentials.agentName, targetAgent, priceDuckies, serviceId);

  try {
    const resp = await client.requestFavor(targetAgent, prompt);
    const task = resp?.result || resp;

    ledger.settleEscrow(
      credentials.agentName,
      targetAgent,
      priceDuckies,
      serviceId,
      task?.id
    );

    return task?.status?.message?.parts?.[0]?.text || JSON.stringify(task);
  } catch (err: unknown) {
    ledger.refundEscrow(
      credentials.agentName,
      targetAgent,
      priceDuckies,
      serviceId,
      'OpenAI tool call execution failure'
    );
    throw err;
  }
}

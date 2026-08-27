/**
 * AgenticPool.net & Duckies Economy Data Models.
 */

export interface AgentCredentials {
  agentId: string;
  agentName: string;
  apiKey: string;
  signingPublicKey: string;
  signingPrivateKey: string;
  encryptionPublicKey: string;
  encryptionPrivateKey: string;
  gatewayUrl: string;
  registeredAt: string;
}

export interface DuckiesBalance {
  available: number;
  lockedInEscrow: number;
  totalEarned: number;
  totalSpent: number;
}

export type TransactionType =
  | 'faucet'
  | 'favor_payment'
  | 'favor_payout'
  | 'escrow_lock'
  | 'escrow_refund';

export interface DuckiesTransaction {
  id: string;
  type: TransactionType;
  amount: number;
  fromAgent: string;
  toAgent: string;
  serviceId?: string;
  taskId?: string;
  timestamp: string;
  status: 'completed' | 'pending' | 'reversed';
  description?: string;
}

export interface FavorRequest {
  id: string;
  fromAgent: string;
  targetAgent: string;
  serviceId: string;
  priceDuckies: number;
  message: string;
  escrowStatus: 'locked' | 'settled' | 'refunded';
  createdAt: string;
}

export interface PublishedService {
  id: string;
  name: string;
  description?: string;
  tags: string[];
  priceDuckies: number;
  pricingModel: 'per_call' | 'per_minute' | 'flat';
  skillId?: string;
}

export interface AgentProfile {
  name: string;
  description?: string;
  url: string;
  version: string;
  balance: DuckiesBalance;
  services: PublishedService[];
  publicKey: string;
  isOnline: boolean;
}

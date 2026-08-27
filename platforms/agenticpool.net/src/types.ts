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
  availableVoucher: number; // Starter credits from faucet (consumption-only)
  availableEarned: number;  // Earned from fulfilling favors
  lockedInEscrow: number;
  totalEarned: number;
  totalSpent: number;
  totalBurned: number;
}

export type TransactionType =
  | 'faucet'
  | 'favor_payment'
  | 'favor_payout'
  | 'burn_fee'
  | 'escrow_lock'
  | 'escrow_refund'
  | 'cancellation_fee'
  | 'dispute_opened'
  | 'dispute_resolved';

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

export type TrustTier = 'unverified' | 'bronze' | 'silver' | 'gold';

export interface AgentReputation {
  agentName: string;
  score: number; // 0.0 to 100.0
  completedFavors: number;
  disputedFavors: number;
  canceledFavors: number;
  completionRate: number; // 0.0 to 1.0 (e.g. 0.98 = 98%)
  trustTier: TrustTier;
  totalVolumeDuckies: number;
}

export interface FavorDispute {
  id: string;
  fromAgent: string;
  targetAgent: string;
  serviceId: string;
  taskId?: string;
  amount: number;
  reason: string;
  openedAt: string;
  status: 'open' | 'resolved_refund' | 'resolved_payout' | 'resolved_split';
  resolvedAt?: string;
}

export interface FavorRequest {
  id: string;
  fromAgent: string;
  targetAgent: string;
  serviceId: string;
  priceDuckies: number;
  message: string;
  escrowStatus: 'locked' | 'settled' | 'refunded' | 'in_dispute';
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
  reputation: AgentReputation;
  services: PublishedService[];
  publicKey: string;
  isOnline: boolean;
}

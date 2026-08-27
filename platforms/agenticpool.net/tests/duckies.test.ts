import { test, describe, after } from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';
import * as os from 'node:os';
import * as fs from 'node:fs';
import { DuckiesLedger } from '../src/economy/duckies.js';

describe('Duckies Economy & Anti-Fraud Suite', () => {
  const ledgerFile = path.join(os.tmpdir(), `duckies_test_ledger_${Date.now()}.json`);
  const disputesFile = path.join(os.tmpdir(), `duckies_test_disputes_${Date.now()}.json`);

  after(() => {
    if (fs.existsSync(ledgerFile)) fs.unlinkSync(ledgerFile);
    if (fs.existsSync(disputesFile)) fs.unlinkSync(disputesFile);
  });

  test('grants starter faucet vouchers and tracks consumption credits separately', () => {
    const ledger = new DuckiesLedger(ledgerFile, disputesFile);
    ledger.grantStarterDuckies('alice', 100);

    const bal = ledger.getBalance('alice');
    assert.equal(bal.available, 100);
    assert.equal(bal.availableVoucher, 100);
    assert.equal(bal.availableEarned, 0);
    assert.equal(bal.lockedInEscrow, 0);
  });

  test('locks escrow consuming voucher first, applies 3% burn fee on settlement', () => {
    const ledger = new DuckiesLedger(ledgerFile, disputesFile);

    // Alice requests favor from Bob for 20 Duckies
    const lockTx = ledger.lockEscrow('alice', 'bob', 20, 'video.render', 'task-1');
    assert.equal(lockTx.type, 'escrow_lock');

    let aliceBal = ledger.getBalance('alice');
    assert.equal(aliceBal.available, 80);
    assert.equal(aliceBal.availableVoucher, 80);
    assert.equal(aliceBal.lockedInEscrow, 20);

    // Favor completes -> Settle escrow with 3% burn fee
    const { payment, burn, payout } = ledger.settleEscrow(
      'alice',
      'bob',
      20,
      'video.render',
      'task-1'
    );

    assert.equal(payment.amount, 20);
    assert.equal(burn.amount, 0.6); // 3% of 20 = 0.6
    assert.equal(payout.amount, 19.4); // 20 - 0.6 = 19.4

    aliceBal = ledger.getBalance('alice');
    assert.equal(aliceBal.available, 80);
    assert.equal(aliceBal.lockedInEscrow, 0);
    assert.equal(aliceBal.totalSpent, 20);
    assert.equal(aliceBal.totalBurned, 0.6);

    const bobBal = ledger.getBalance('bob');
    assert.equal(bobBal.available, 19.4);
    assert.equal(bobBal.availableEarned, 19.4);
    assert.equal(bobBal.totalEarned, 19.4);
  });

  test('compensates worker with 10% compute fee on in-progress cancellation', () => {
    const ledger = new DuckiesLedger(ledgerFile, disputesFile);

    ledger.lockEscrow('alice', 'charlie', 50, 'ai.inference');
    let aliceBal = ledger.getBalance('alice');
    assert.equal(aliceBal.available, 30);
    assert.equal(aliceBal.lockedInEscrow, 50);

    // Alice cancels mid-flight -> 10% compensation to Charlie, 90% refund to Alice
    const { compensation, refund } = ledger.cancelEscrowWithCompensation(
      'alice',
      'charlie',
      50,
      'ai.inference'
    );

    assert.equal(compensation.amount, 5); // 10% of 50 = 5
    assert.equal(refund.amount, 45); // 90% of 50 = 45

    const charlieBal = ledger.getBalance('charlie');
    assert.equal(charlieBal.available, 5);
    assert.equal(charlieBal.availableEarned, 5);

    aliceBal = ledger.getBalance('alice');
    assert.equal(aliceBal.available, 75); // 30 remaining + 45 refunded
    assert.equal(aliceBal.lockedInEscrow, 0);
  });

  test('validates task outputs against fraudulent empty responses', () => {
    const ledger = new DuckiesLedger(ledgerFile, disputesFile);

    assert.equal(ledger.validateOutput(null).valid, false);
    assert.equal(ledger.validateOutput('').valid, false);
    assert.equal(ledger.validateOutput('   ').valid, false);
    assert.equal(ledger.validateOutput('Error: execution crashed').valid, false);
    assert.equal(ledger.validateOutput('Valid translation result').valid, true);
  });

  test('opens and resolves disputes with refund', () => {
    const ledger = new DuckiesLedger(ledgerFile, disputesFile);

    const dispute = ledger.openDispute(
      'alice',
      'bad-agent',
      10,
      'code.debug',
      'Output was empty string'
    );
    assert.equal(dispute.status, 'open');

    const resolved = ledger.resolveDispute(dispute.id, 'refund');
    assert.equal(resolved.status, 'resolved_refund');
  });

  test('calculates agent reputation score and trust tier', () => {
    const ledger = new DuckiesLedger(ledgerFile, disputesFile);

    // Bob has 1 completed favor from earlier test
    const bobRep = ledger.getReputation('bob');
    assert.equal(bobRep.agentName, 'bob');
    assert.equal(bobRep.completedFavors, 1);
    assert.equal(bobRep.completionRate, 1.0);
    assert.equal(bobRep.trustTier, 'unverified'); // needs >= 5 for bronze
    assert.ok(bobRep.score > 0);
  });
});

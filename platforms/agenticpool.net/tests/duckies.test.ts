import { test, describe, after } from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';
import * as os from 'node:os';
import * as fs from 'node:fs';
import { DuckiesLedger } from '../src/economy/duckies.js';

describe('Duckies Economy & Escrow', () => {
  const ledgerFile = path.join(os.tmpdir(), `duckies_test_${Date.now()}.json`);

  after(() => {
    if (fs.existsSync(ledgerFile)) {
      fs.unlinkSync(ledgerFile);
    }
  });

  test('grants starter faucet Duckies and calculates balance', () => {
    const ledger = new DuckiesLedger(ledgerFile);
    ledger.grantStarterDuckies('alice', 100);

    const bal = ledger.getBalance('alice');
    assert.equal(bal.available, 100);
    assert.equal(bal.lockedInEscrow, 0);
  });

  test('locks escrow, settles favor payout, and updates ledger', () => {
    const ledger = new DuckiesLedger(ledgerFile);

    // Alice requests favor from Bob for 20 Duckies
    const lockTx = ledger.lockEscrow('alice', 'bob', 20, 'video.render', 'task-1');
    assert.equal(lockTx.type, 'escrow_lock');

    let aliceBal = ledger.getBalance('alice');
    assert.equal(aliceBal.available, 80);
    assert.equal(aliceBal.lockedInEscrow, 20);

    // Favor completes -> Settle escrow
    ledger.settleEscrow('alice', 'bob', 20, 'video.render', 'task-1');

    aliceBal = ledger.getBalance('alice');
    assert.equal(aliceBal.available, 80);
    assert.equal(aliceBal.lockedInEscrow, 0);
    assert.equal(aliceBal.totalSpent, 20);

    const bobBal = ledger.getBalance('bob');
    assert.equal(bobBal.available, 20);
    assert.equal(bobBal.totalEarned, 20);
  });

  test('refunds escrow if favor fails', () => {
    const ledger = new DuckiesLedger(ledgerFile);

    ledger.lockEscrow('alice', 'charlie', 15, 'code.debug');
    let aliceBal = ledger.getBalance('alice');
    assert.equal(aliceBal.available, 65);
    assert.equal(aliceBal.lockedInEscrow, 15);

    // Refund
    ledger.refundEscrow('alice', 'charlie', 15, 'code.debug', 'Charlie offline');

    aliceBal = ledger.getBalance('alice');
    assert.equal(aliceBal.available, 80);
    assert.equal(aliceBal.lockedInEscrow, 0);
  });
});

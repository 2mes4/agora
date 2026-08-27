import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import { AgentCard, AgentService, ServicePricing } from '../src/types.js';

describe('Types & Models', () => {
  test('AgentService with ServicePricing structure', () => {
    const pricing: ServicePricing = {
      amount: 12.5,
      currency: 'EUR',
      model: 'per_call',
    };

    const service: AgentService = {
      id: 'render.hd',
      name: 'HD Rendering Service',
      description: 'Generates 1080p video renders',
      tags: ['video', 'render', 'hd'],
      pricing,
      skillId: 'video_generation',
    };

    const card: AgentCard = {
      name: 'video_agent',
      description: 'Expert video creator',
      version: '0.1.0',
      url: 'http://127.0.0.1:7105',
      services: [service],
    };

    assert.equal(card.name, 'video_agent');
    assert.equal(card.services?.length, 1);
    assert.equal(card.services[0].pricing.amount, 12.5);
    assert.equal(card.services[0].pricing.currency, 'EUR');
    assert.equal(card.services[0].pricing.model, 'per_call');
  });
});

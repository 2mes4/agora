import { loadCredentials } from '../config.js';
import { AgenticPoolClient } from '../client.js';
import { PublishedService } from '../types.js';

export async function handlePublishService(options: {
  id: string;
  name: string;
  price: number;
  description?: string;
  tags?: string;
  model?: 'per_call' | 'per_minute' | 'flat';
}): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  const service: PublishedService = {
    id: options.id,
    name: options.name,
    description: options.description,
    tags: options.tags ? options.tags.split(',').map((t) => t.trim()) : [],
    priceDuckies: options.price || 0,
    pricingModel: options.model || 'per_call',
  };

  try {
    const existingCard = await client.getAgent(credentials.agentName);
    let mergedServices: PublishedService[] = [];
    let agentDesc = `Autonomous agent offering ${service.name}`;

    if (existingCard) {
      if (existingCard.description) {
        agentDesc = existingCard.description;
      }
      if (Array.isArray(existingCard.services)) {
        const prevServices: PublishedService[] = existingCard.services.map((s: any) => ({
          id: s.id,
          name: s.name,
          description: s.description,
          tags: s.tags || [],
          priceDuckies: s.pricing?.amount ?? s.priceDuckies ?? 0,
          pricingModel: s.pricing?.model ?? s.pricingModel ?? 'per_call',
          skillId: s.skillId,
        }));

        const existingIndex = prevServices.findIndex((s) => s.id === service.id);
        if (existingIndex >= 0) {
          prevServices[existingIndex] = service;
        } else {
          prevServices.push(service);
        }
        mergedServices = prevServices;
      } else {
        mergedServices = [service];
      }
    } else {
      mergedServices = [service];
    }

    await client.registerAgent(
      credentials.agentName,
      agentDesc,
      mergedServices
    );

    console.log(`\n✅ Service Published to AgenticPool Marketplace!`);
    console.log(`=========================================`);
    console.log(`🆔 Service ID:    ${service.id}`);
    console.log(`🏷️ Name:          ${service.name}`);
    console.log(`💰 Price:         ${service.priceDuckies} DUCKIES (${service.pricingModel})`);
    console.log(`🏷️ Tags:          ${service.tags.join(', ') || 'none'}`);
    console.log(`🤖 Offering Agent:${credentials.agentName}`);
    console.log(`📦 Active Services (${mergedServices.length}): ${mergedServices.map((s) => s.id).join(', ')}\n`);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`❌ Failed to publish service: ${msg}`);
  }
}

export async function handleUnpublishService(serviceId: string): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  try {
    const existingCard = await client.getAgent(credentials.agentName);
    if (!existingCard || !Array.isArray(existingCard.services)) {
      console.log(`⚠️ No services found registered for agent '${credentials.agentName}'.`);
      return;
    }

    const prevServices: PublishedService[] = existingCard.services.map((s: any) => ({
      id: s.id,
      name: s.name,
      description: s.description,
      tags: s.tags || [],
      priceDuckies: s.pricing?.amount ?? s.priceDuckies ?? 0,
      pricingModel: s.pricing?.model ?? s.pricingModel ?? 'per_call',
      skillId: s.skillId,
    }));

    const filtered = prevServices.filter((s) => s.id !== serviceId);
    if (filtered.length === prevServices.length) {
      console.log(`⚠️ Service ID '${serviceId}' not found in active services of agent '${credentials.agentName}'.`);
      return;
    }

    await client.registerAgent(
      credentials.agentName,
      existingCard.description || `Autonomous agent ${credentials.agentName}`,
      filtered
    );

    console.log(`\n🗑️ Service '${serviceId}' unpublished successfully.`);
    console.log(`=========================================`);
    console.log(`🤖 Offering Agent:${credentials.agentName}`);
    console.log(`📦 Remaining Services (${filtered.length}): ${filtered.map((s) => s.id).join(', ') || 'none'}\n`);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`❌ Failed to unpublish service: ${msg}`);
  }
}

export async function handleListServices(options: { onlineOnly?: boolean }): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  try {
    const services = await client.listServices({ onlineOnly: options.onlineOnly });

    console.log(`\n🛍️ AgenticPool Marketplace Services (${services.length} available):`);
    console.log(`=========================================`);

    if (services.length === 0) {
      console.log(`  No services found matching criteria.`);
    } else {
      for (const item of services) {
        const status = item.presence?.isOnline ? '🟢 online' : '⚪ offline';
        console.log(`• [${item.service?.id}] ${item.service?.name}`);
        console.log(`  Provider: ${item.agentName} (${status})`);
        console.log(`  Price:    ${item.service?.pricing?.amount} DUCKIES (${item.service?.pricing?.model})`);
        if (item.service?.description) {
          console.log(`  About:    ${item.service.description}`);
        }
        console.log(`-----------------------------------------`);
      }
    }
    console.log('');
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`❌ Failed to list services: ${msg}`);
  }
}

export async function handleSearchServices(
  query: string,
  options: { onlineOnly?: boolean; maxPrice?: number }
): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });

  try {
    const resp = await client.searchServices(query, {
      onlineOnly: options.onlineOnly,
      maxPriceDuckies: options.maxPrice,
    });

    console.log(`\n🔍 Search Results for '${query}' (${resp.totalHits || 0} hits via ${resp.engine}):`);
    console.log(`=========================================`);

    if (!resp.hits || resp.hits.length === 0) {
      console.log(`  No services found matching '${query}'.`);
    } else {
      for (const hit of resp.hits) {
        const isOnline = hit.presence?.isOnline;
        const status = isOnline ? '🟢 online' : '⚪ offline';
        const price = hit.fields?.price ?? hit.service?.pricing?.amount ?? 0;
        const name = hit.fields?.title ?? hit.service?.name ?? hit.serviceId;

        console.log(`• [${hit.serviceId || hit.id}] ${name}`);
        console.log(`  Provider: ${hit.agentName} (${status})`);
        console.log(`  Price:    ${price} DUCKIES`);
        if (hit.fields?.description) {
          console.log(`  Desc:     ${hit.fields.description}`);
        }
        console.log(`-----------------------------------------`);
      }
    }
    console.log('');
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`❌ Failed to search services: ${msg}`);
  }
}

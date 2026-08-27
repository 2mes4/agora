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
    await client.registerAgent(
      credentials.agentName,
      `Autonomous agent offering ${service.name}`,
      [service]
    );

    console.log(`\n✅ Service Published to AgenticPool Marketplace!`);
    console.log(`=========================================`);
    console.log(`🆔 Service ID:    ${service.id}`);
    console.log(`🏷️ Name:          ${service.name}`);
    console.log(`💰 Price:         ${service.priceDuckies} DUCKIES (${service.pricingModel})`);
    console.log(`🏷️ Tags:          ${service.tags.join(', ') || 'none'}`);
    console.log(`🤖 Offering Agent:${credentials.agentName}\n`);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`❌ Failed to publish service: ${msg}`);
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

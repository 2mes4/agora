import { loadCredentials } from '../config.js';
import { AgenticPoolClient } from '../client.js';

export async function handleTrustEvaluate(target: string, fromAgent?: string): Promise<void> {
  const credentials = loadCredentials();
  const evaluator = fromAgent || credentials.agentName;
  const client = new AgenticPoolClient({ credentials });

  console.log(`\n🕸️  Evaluating Trust in '${target}' from perspective of '${evaluator}'...`);
  const evalResult = await client.evaluateTrust(target, evaluator);

  console.log(`\n======================================================`);
  console.log(`📊 Perspectivist Trust Report for '${target}'`);
  console.log(`======================================================`);
  console.log(`🌐 Global Network Metrics:`);
  console.log(`   • Duckies de Goma (Éxitos):    🦆 ${evalResult.globalMetrics.gomaTotal}`);
  console.log(`   • Duckies de Plomo (Fallos):   🌑 ${evalResult.globalMetrics.plomoTotal.toFixed(1)}`);
  console.log(`   • Nodos Conectados:            🔗 ${evalResult.globalMetrics.connections}`);
  console.log(`   • Score General:               ⭐ ${evalResult.globalMetrics.score.toFixed(2)}`);
  console.log(`   • Ratio Global de Éxito:       🎯 ${(evalResult.globalMetrics.ratio * 100).toFixed(1)}%\n`);

  if (evalResult.personalizedTrust) {
    const pt = evalResult.personalizedTrust;
    console.log(`👤 Según tu red ('${evaluator}'):`);
    if (pt.killSwitchActive) {
      console.log(`   ⛔ ESTADO: ¡KILL SWITCH ACTIVADO! (Goma <= Plomo en historial local)`);
      console.log(`   🚫 Credibilidad: 0.0% (Veto Inapelable - No enrutar tráfico)`);
    } else {
      let icon = '🟢';
      if (pt.verdict === 'cautious') icon = '🟡';
      console.log(`   • Veredicto:                   ${icon} ${pt.verdict.toUpperCase()}`);
      console.log(`   • Credibilidad Recomendada:    🎯 ${pt.credibilityPercent.toFixed(1)}%`);
    }

    if (pt.directInteractions.hasHistory) {
      console.log(
        `   • Historial Directo (1 salto): 🦆 ${pt.directInteractions.gomaLocal} Goma | 🌑 ${pt.directInteractions.plomoLocal.toFixed(1)} Plomo`
      );
    } else {
      console.log(`   • Historial Directo (1 salto): (Sin interacciones previas)`);
    }

    if (pt.networkVouching.trustedPeersCount > 0) {
      console.log(
        `   • Avalado por tu red (2 saltos): ${pt.networkVouching.trustedPeersCount} agentes (${pt.networkVouching.samplePeers.join(', ')})`
      );
    }
  }
  console.log(`======================================================\n`);
}

export async function handleTrustRecord(
  target: string,
  goma = 0,
  plomo = 0.0,
  fromAgent?: string
): Promise<void> {
  const credentials = loadCredentials();
  const evaluator = fromAgent || credentials.agentName;
  const client = new AgenticPoolClient({ credentials });

  const edge = await client.recordTrust({
    fromAgent: evaluator,
    toAgent: target,
    goma,
    plomo,
  });

  console.log(`\n✅ Interacción registrada en el Grafo de Confianza:`);
  console.log(`   • De:              ${edge.fromAgent}`);
  console.log(`   • Hacia:           ${edge.toAgent}`);
  console.log(`   • Total Goma:      🦆 ${edge.goma}`);
  console.log(`   • Total Plomo:     🌑 ${edge.plomo.toFixed(1)}\n`);
}

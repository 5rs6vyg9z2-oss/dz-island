import type { ZeusBatteryEstimate } from '../hooks/useZeusBattery';

export function batteryStatus(state: ZeusBatteryEstimate): string {
  switch (state.source) {
    case 'bluetooth':
      if (state.charging) return 'Carregando · nível informado pelo Windows';
      if (state.level_increased) return 'Nível subiu · base atualizada pelo Bluetooth';
      return 'Nível informado pelo Windows · sincronização automática';
    case 'bluetooth-unavailable':
      return 'Bluetooth conectado · aguardando leitura do Windows';
    case 'estimate': {
      if (!state.initialized) return 'Conecte por Bluetooth para calibrar automaticamente.';
      const minutes = Math.ceil(state.remaining_seconds / 60);
      const activity = state.running ? 'Contando automaticamente' : 'Estimativa pausada';
      return `${activity} · ≈ ${Math.floor(minutes / 60)} h ${minutes % 60} min`;
    }
    default:
      return 'Verificando conexão Bluetooth…';
  }
}

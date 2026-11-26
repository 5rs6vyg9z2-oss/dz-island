import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Headphones } from 'lucide-react';
import './ZeusEstimate.css';

interface Estimate {
  initialized: boolean; hours: number; used_seconds: number;
  running: boolean; percent: number; remaining_seconds: number;
  source: 'checking' | 'bluetooth' | 'bluetooth-unavailable' | 'estimate';
  charging: boolean; level_increased: boolean; manual_pause: boolean;
}

export function ZeusEstimate() {
  const [state, setState] = useState<Estimate | null>(null);
  const [hours, setHours] = useState('18');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    let disposed = false;
    const refresh = (initial = false) => invoke<Estimate>('zeus_estimate_status').then(value => {
      if (!disposed) { setState(value); if (initial) setHours(String(value.hours)); }
    }).catch(() => { if (!disposed) setError('Não foi possível ler a estimativa.'); });
    void refresh(true);
    const timer = window.setInterval(() => void refresh(), 15000);
    return () => { disposed = true; window.clearInterval(timer); };
  }, []);
  const update = async (action: string) => {
    if (busy) return;
    setBusy(true); setError('');
    try {
      const value = await invoke<Estimate>('zeus_estimate_update', {
        action, hours: action === 'hours' ? Number(hours) : null,
      });
      setState(value); setHours(String(value.hours));
    } catch { setError('Não foi possível salvar. Confira a autonomia (1–100 h).'); }
    finally { setBusy(false); }
  };
  const left = state ? Math.ceil(state.remaining_seconds / 60) : 0;
  const bluetooth = state?.source === 'bluetooth';
  const estimating = state?.source === 'estimate';
  const showPercent = state?.initialized && (bluetooth || estimating);
  const status = bluetooth
    ? state.charging ? 'Carregando · nível informado pelo Windows'
      : state.level_increased ? 'Nível subiu · base atualizada pelo Bluetooth' : 'Nível informado pelo Windows · sincronização automática'
    : state?.source === 'bluetooth-unavailable' ? 'Bluetooth conectado · aguardando leitura do Windows'
      : !estimating ? 'Verificando conexão Bluetooth…'
        : state?.initialized ? `${state.running ? 'Contando automaticamente' : 'Estimativa pausada'} · ≈ ${Math.floor(left / 60)} h ${left % 60} min`
          : 'Conecte por Bluetooth para calibrar automaticamente.';
  return <section className="zeus-estimate" aria-label="Bateria do Zeus Pro" onClick={event => event.stopPropagation()}>
    <header><Headphones size={17} /><b>Zeus Pro</b><span>{bluetooth ? 'Bluetooth · Windows' : 'Estimativa'}</span>
      <strong>{showPercent ? `${bluetooth ? '' : '≈ '}${state.percent}%` : '—'}</strong></header>
    <p aria-live="polite">{status}</p>
    <div className="zeus-estimate-actions">
      <button disabled={busy || !state?.initialized || !estimating} onClick={() => void update(state?.running ? 'pause' : 'start')}>
        {state?.running ? 'Pausar' : 'Retomar estimativa'}</button>
      <button disabled={busy || !state || !estimating} onClick={() => void update('full')}>Carreguei totalmente</button>
    </div>
    <div className="zeus-estimate-capacity"><label>Autonomia <input aria-label="Autonomia estimada em horas" type="number" min="1" max="100" step="1"
      value={hours} onChange={event => setHours(event.target.value)} onPointerDown={event => event.stopPropagation()} /> h</label>
      <button disabled={busy || !state || !Number.isInteger(Number(hours)) || Number(hours) < 1 || Number(hours) > 100 || Number(hours) === state.hours}
        onClick={() => void update('hours')}>Salvar</button></div>
    <small>Bluetooth atualiza a base automaticamente. Só no receptor, pause ao desligar ou carregar: ele não informa esses estados.</small>
    {error && <small role="alert">{error}</small>}
  </section>;
}

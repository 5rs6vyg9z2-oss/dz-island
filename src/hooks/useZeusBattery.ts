import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

const REFRESH_INTERVAL_MS = 15_000;
export const MIN_AUTONOMY_HOURS = 1;
export const MAX_AUTONOMY_HOURS = 100;

type EstimateAction = 'hours' | 'full' | 'pause' | 'start';

export interface ZeusBatteryEstimate {
  detected: boolean;
  initialized: boolean;
  hours: number;
  used_seconds: number;
  running: boolean;
  percent: number;
  remaining_seconds: number;
  source: 'absent' | 'checking' | 'bluetooth' | 'bluetooth-unavailable' | 'estimate';
  charging: boolean;
  level_increased: boolean;
  manual_pause: boolean;
}

export function useZeusBattery() {
  const [state, setState] = useState<ZeusBatteryEstimate | null>(null);
  const [hours, setHours] = useState('18');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    let disposed = false;
    const refresh = async (initial = false) => {
      try {
        const value = await invoke<ZeusBatteryEstimate>('zeus_estimate_status');
        if (disposed) return;
        setState(value);
        if (initial) setHours(String(value.hours));
      } catch {
        if (!disposed) setError('Não foi possível ler a estimativa.');
      }
    };
    void refresh(true);
    const timer = window.setInterval(() => void refresh(), REFRESH_INTERVAL_MS);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, []);

  const update = async (action: EstimateAction) => {
    if (busy) return;
    setBusy(true);
    setError('');
    try {
      const value = await invoke<ZeusBatteryEstimate>('zeus_estimate_update', {
        action,
        hours: action === 'hours' ? Number(hours) : null,
      });
      setState(value);
      setHours(String(value.hours));
    } catch {
      setError('Não foi possível salvar. Confira a autonomia (1–100 h).');
    } finally {
      setBusy(false);
    }
  };

  const autonomy = Number(hours);
  const canSaveHours = !busy && state !== null
    && Number.isInteger(autonomy)
    && autonomy >= MIN_AUTONOMY_HOURS
    && autonomy <= MAX_AUTONOMY_HOURS
    && autonomy !== state.hours;

  return { state, hours, setHours, busy, error, update, canSaveHours };
}

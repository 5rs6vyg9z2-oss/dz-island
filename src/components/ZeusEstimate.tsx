import { useLayoutEffect, useRef } from 'react';
import { Headphones } from 'lucide-react';
import { MAX_AUTONOMY_HOURS, MIN_AUTONOMY_HOURS, useZeusBattery } from '../hooks/useZeusBattery';
import { batteryStatus } from './zeusBatteryStatus';
import './ZeusEstimate.css';

interface ZeusEstimateProps {
  onHeightChange: (height: number) => void;
}

export function ZeusEstimate({ onHeightChange }: ZeusEstimateProps) {
  const section = useRef<HTMLDetailsElement>(null);
  const { state, hours, setHours, busy, error, update, canSaveHours } = useZeusBattery();

  useLayoutEffect(() => {
    const element = section.current;
    if (!element) {
      onHeightChange(0);
      return;
    }
    const measure = () => onHeightChange(Math.ceil(element.getBoundingClientRect().height) + 12);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => {
      observer.disconnect();
      onHeightChange(0);
    };
  }, [state?.detected, onHeightChange]);

  const bluetooth = state?.source === 'bluetooth';
  const estimating = state?.source === 'estimate';
  const showPercent = state?.initialized && (bluetooth || estimating);

  if (!state?.detected) return null;
  return (
    <details
      ref={section}
      className="zeus-estimate"
      aria-label="Bateria do Zeus Pro"
      onClick={event => event.stopPropagation()}
    >
      <summary>Opções de bateria</summary>
      <header>
        <Headphones size={17} />
        <b>Zeus Pro</b>
        <span>{bluetooth ? 'Bluetooth · Windows' : 'Estimativa'}</span>
        <strong>{showPercent ? `${bluetooth ? '' : '≈ '}${state.percent}%` : '—'}</strong>
      </header>
      <p aria-live="polite">{batteryStatus(state)}</p>
      <div className="zeus-estimate-actions">
        <button
          disabled={busy || !state.initialized || !estimating}
          onClick={() => void update(state.running ? 'pause' : 'start')}
        >
          {state.running ? 'Pausar' : 'Retomar estimativa'}
        </button>
        <button disabled={busy || !estimating} onClick={() => void update('full')}>
          Carreguei totalmente
        </button>
      </div>
      <div className="zeus-estimate-capacity">
        <label>
          Autonomia <input
            aria-label="Autonomia estimada em horas"
            type="number"
            min={MIN_AUTONOMY_HOURS}
            max={MAX_AUTONOMY_HOURS}
            step="1"
            value={hours}
            onChange={event => setHours(event.target.value)}
            onPointerDown={event => event.stopPropagation()}
          /> h
        </label>
        <button disabled={!canSaveHours} onClick={() => void update('hours')}>
          Salvar
        </button>
      </div>
      <small>
        Bluetooth atualiza a base automaticamente. Só no receptor, pause ao desligar ou carregar:
        ele não informa esses estados.
      </small>
      {error && <small role="alert">{error}</small>}
    </details>
  );
}

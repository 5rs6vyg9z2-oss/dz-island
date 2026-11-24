import { useEffect, useState } from 'react';
import { AgentPanel, type AgentActivity } from './AgentStatus';

export function CodexCompanion({ activity, onAnswer }: {
  activity: AgentActivity | null;
  onAnswer: (allow: boolean, choices?: number[]) => Promise<void>;
}) {
  const [picks, setPicks] = useState<number[]>([]);
  const [sending, setSending] = useState(false);
  const [feedback, setFeedback] = useState('');
  const [error, setError] = useState('');
  const askId = activity?.ask?.id;
  useEffect(() => {
    setPicks([]);
    setError('');
    if (askId !== undefined) setFeedback('');
  }, [askId]);
  const answer = async (allow: boolean, choices?: number[]) => {
    if (sending) return;
    setSending(true); setError('');
    try {
      await onAnswer(allow, choices);
      setFeedback(allow ? 'Resposta enviada ao Codex.' : 'Pergunta dispensada.');
    } catch {
      setError('Não foi possível enviar. Tente novamente.');
    } finally { setSending(false); }
  };
  const status = askId !== undefined ? 'Sua resposta' : activity?.state === 'working' || activity?.state === 'tool' ? 'Trabalhando' : activity?.state === 'done' ? 'Concluído' : 'Conectado';
  return (
    <section className={`codex-companion is-open ${askId !== undefined ? 'needs-answer' : ''}`}>
      <div className="codex-companion-toggle">
        <span className="codex-companion-mark" aria-hidden="true">✳</span>
        <span><b>Codex</b><small>{status}</small></span>
      </div>
      <div className="codex-companion-body" aria-live="polite">
        {activity?.ask ? <>
          <fieldset disabled={sending} className="codex-companion-question">
            <AgentPanel activity={activity} picks={picks} onPick={setPicks} onOpen={() => {}} onDismiss={() => {}} onAnswer={(allow, choices) => { void answer(allow, choices); }} />
          </fieldset>
          <button className="codex-companion-dismiss" disabled={sending} onClick={() => { void answer(false); }}>Dispensar pergunta</button>
        </> : <p>{feedback || activity?.detail || 'Pronto para receber perguntas deste projeto. As opções aparecerão aqui automaticamente.'}</p>}
        {error && <p role="alert">{error}</p>}
      </div>
    </section>
  );
}

import { useEffect, useState } from 'react';
import { AgentMascot, AgentPanel, type AgentActivity } from './AgentStatus';

export function CodexCompanion({ activity, onAnswer, onDismiss }: {
  activity: AgentActivity | null;
  onAnswer: (allow: boolean, choices?: number[]) => Promise<void>;
  onDismiss?: () => void;
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
  const status = askId !== undefined ? (activity?.finished ? 'Concluído · resposta pendente' : 'Resposta pendente') : activity?.state === 'working' || activity?.state === 'tool' ? 'Trabalhando' : activity?.state === 'done' ? 'Concluído' : activity?.state === 'error' ? 'Erro no agente' : activity?.state === 'needs_input' ? 'Precisa da sua atenção' : 'Conectado';
  return (
    <section className={`codex-companion is-open ${askId !== undefined ? 'needs-answer' : ''}`}>
      <div className="codex-companion-toggle">
        <span className="codex-companion-mark" aria-hidden="true">{activity?.ask ? <AgentMascot activity={activity} /> : '✳'}</span>
        <span><b>{activity?.name || 'Codex'}</b><small>{status}</small></span>
      </div>
      <div className="codex-companion-body" aria-live="polite">
        {activity?.ask ? <>
          <p className="codex-pending-note">Aguardando você. Esta pergunta fica aqui até ser respondida ou dispensada.</p>
          <fieldset disabled={sending} className="codex-companion-question">
            <AgentPanel activity={activity} picks={picks} onPick={setPicks} onOpen={() => {}} onDismiss={() => {}} onAnswer={(allow, choices) => { void answer(allow, choices); }} />
          </fieldset>
          <button className="codex-companion-dismiss" disabled={sending} onClick={() => { void answer(false); }}>Dispensar pergunta</button>
        </> : <p>{feedback || activity?.detail || 'Pronto para receber perguntas deste projeto. As opções aparecerão aqui automaticamente.'}</p>}
        {!activity?.ask && onDismiss && <button className="codex-companion-dismiss" onClick={onDismiss}>Fechar aviso</button>}
        {error && <p role="alert">{error}</p>}
      </div>
    </section>
  );
}

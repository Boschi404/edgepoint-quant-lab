import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import {
  ApiError, commandRun, connectProgress, createRun, getEquity, getMetrics, getRanking,
  getRun, getSearchState, getTrades, getValidation, listArtifacts, listRuns, recoverRun,
  type SocketStatus,
} from './api';
import type { ArtifactEntry, ProgressEvent, RunStatus, RunSummary } from './types';
import './styles.css';

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

const OP_LABELS: Record<string, string> = {
  'list runs': 'caricamento della lista dei run',
  'create run': 'creazione del nuovo run',
  'load run': 'caricamento dello stato del run',
  'pause run': 'invio del comando Pause',
  'resume run': 'invio del comando Resume',
  'cancel run': 'invio del comando Cancel',
  'recover run': 'ripristino del run',
  'load ranking': 'caricamento del ranking',
  'load artifacts': 'caricamento degli artifact',
  'load metrics': 'caricamento delle metriche',
  'load trades': 'caricamento dei trade',
  'load equity': 'caricamento dell\u2019equity',
  'load search state': 'caricamento dello stato di ricerca',
  'load validation': 'caricamento della validazione',
};

function describeError(err: unknown): string {
  if (err instanceof ApiError) {
    const op = OP_LABELS[err.op] ?? err.op;
    if (err.status === 0) return `${op}: server API non raggiungibile`;
    if (err.status === 404) return `${op}: risorsa non trovata`;
    if (err.status >= 500) return `${op}: errore lato server (${err.status})`;
    return `${op} non riuscito (${err.status})`;
  }
  return err instanceof Error ? err.message : String(err);
}

function formatClock(timestamp: number): string {
  const date = new Date(timestamp);
  return Number.isNaN(date.getTime())
    ? ''
    : date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function formatDateTime(timestamp: number): string {
  const date = new Date(timestamp);
  return Number.isNaN(date.getTime())
    ? '?'
    : date.toLocaleString([], { dateStyle: 'short', timeStyle: 'short' });
}

function shortId(id: string): string {
  return id.length > 18 ? `${id.slice(0, 8)}\u2026${id.slice(-6)}` : id;
}

/** Which lifecycle commands are legal for a run in this state. */
function allowedCommands(state: string): Set<'pause' | 'resume' | 'cancel' | 'recover'> {
  switch (state as RunStatus) {
    case 'Pending': return new Set(['cancel']);
    case 'Running': return new Set(['pause', 'cancel']);
    case 'Paused': return new Set(['resume', 'cancel']);
    case 'Failed':
    case 'Cancelled': return new Set(['recover']);
    default: return new Set();
  }
}

interface EventRow extends ProgressEvent {
  key: string;
}

/* ------------------------------------------------------------------ */
/* App                                                                 */
/* ------------------------------------------------------------------ */

function App() {
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedState, setSelectedState] = useState<string | null>(null);

  const [events, setEvents] = useState<EventRow[]>([]);
  const [socketStatus, setSocketStatus] = useState<SocketStatus>('connecting');

  const [ranking, setRanking] = useState<unknown>(null);
  const [artifacts, setArtifacts] = useState<ArtifactEntry[]>([]);
  const [metrics, setMetrics] = useState<unknown[]>([]);
  const [trades, setTrades] = useState<unknown[]>([]);
  const [equity, setEquity] = useState<unknown[]>([]);
  const [searchState, setSearchState] = useState<unknown>(null);
  const [validation, setValidation] = useState<unknown>(null);

  const [banner, setBanner] = useState<{ kind: 'error' | 'info'; text: string } | null>(null);
  const [runsError, setRunsError] = useState(false);
  const [creating, setCreating] = useState(false);
  const [loadingPanels, setLoadingPanels] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const eventSeq = useRef(0);
  const lastEventStatus = useRef<string | null>(null);
  const socketStatusRef = useRef<SocketStatus>('connecting');

  const showError = useCallback((err: unknown) => {
    setBanner({ kind: 'error', text: describeError(err) });
  }, []);

  const handleSocketState = useCallback((status: SocketStatus) => {
    socketStatusRef.current = status;
    setSocketStatus(status);
  }, []);

  /* ---- runs list ------------------------------------------------- */

  const refresh = useCallback(async (): Promise<void> => {
    try {
      const list = await listRuns();
      setRuns(list);
      setRunsError(false);
      setSelectedId((current) => {
        if (!current) return current;
        const match = list.find((r) => r.run_id === current);
        if (match) setSelectedState(match.state);
        return current;
      });
    } catch (err) {
      setRunsError(true);
      showError(err);
    }
  }, [showError]);

  useEffect(() => { void refresh(); }, [refresh]);

  /* ---- selection + realtime feed --------------------------------- */

  useEffect(() => {
    setEvents([]);
    setRanking(null);
    setArtifacts([]);
    setMetrics([]);
    setTrades([]);
    setEquity([]);
    setSearchState(null);
    setValidation(null);
    setSelectedState(null);
    lastEventStatus.current = null;
    if (!selectedId) return;

    let alive = true;
    let connection: { close(): void } | null = null;
    let pollTimer: ReturnType<typeof setInterval> | undefined;

    // Safety net while the socket is not healthy: poll the run state so a
    // stalled feed still shows movement instead of freezing silently.
    pollTimer = setInterval(() => {
      if (!alive || !selectedId) return;
      if (socketStatusRef.current !== 'connected') void getRun(selectedId).then((r) => {
        if (!alive) return;
        setSelectedState(r.state);
      }).catch(() => undefined);
    }, 5000);

    const loadResults = (runId: string): void => {
      setLoadingPanels(true);
      const jobs: Array<[string, Promise<void>]> = [
        ['ranking', getRanking(runId).then((v) => { if (alive) setRanking(v); })],
        ['artifacts', listArtifacts(runId).then((v) => { if (alive) setArtifacts(v); })],
        ['metrics', getMetrics(runId).then((v) => { if (alive) setMetrics(v); })],
        ['trades', getTrades(runId).then((v) => { if (alive) setTrades(v); })],
        ['equity', getEquity(runId).then((v) => { if (alive) setEquity(v); })],
        ['search state', getSearchState(runId).then((v) => { if (alive) setSearchState(v); })],
        ['validation', getValidation(runId).then((v) => { if (alive) setValidation(v); })],
      ];
      void Promise.allSettled(jobs.map(([op, p]) => p.catch((err) => { throw new ApiError(op, err instanceof ApiError ? err.status : 0); })))
        .then((results) => {
          if (alive) setLoadingPanels(false);
          const failed = results.filter((r) => r.status === 'rejected').map((r) => OP_LABELS[String((r as PromiseRejectedResult).reason?.op)] ?? '');
          if (failed.length > 0) {
            setBanner({ kind: 'error', text: `Risultati parzialmente caricati; non riusciti: ${failed.filter(Boolean).join(', ')}` });
          }
        });
    };

    connection = connectProgress(selectedId, {
      onState: handleSocketState,
      onEvent: (raw) => {
        if (!alive) return;
        const row: EventRow = { ...raw, key: `evt-${++eventSeq.current}` };
        setEvents((prev) => [row, ...prev].slice(0, 200));
        setSelectedState(raw.status);
        if (raw.status !== lastEventStatus.current) {
          lastEventStatus.current = raw.status;
          void refresh();
          if (raw.status === 'Completed') loadResults(selectedId);
        }
      },
    });

    return () => {
      alive = false;
      if (pollTimer !== undefined) clearInterval(pollTimer);
      connection?.close();
    };
  }, [selectedId, refresh]);

  /* ---- actions ---------------------------------------------------- */

  async function startRun(): Promise<void> {
    if (creating) return;
    setCreating(true);
    try {
      const run = await createRun();
      await refresh();
      setSelectedId(run.run_id);
      setBanner(null);
    } catch (err) {
      showError(err);
    } finally {
      setCreating(false);
    }
  }

  async function sendCommand(command: 'pause' | 'resume' | 'cancel'): Promise<void> {
    if (!selectedId || busy) return;
    if (command === 'cancel'
      && !window.confirm(`Annullare definitivamente il run ${shortId(selectedId)}? I progressi non salvati vanno persi.`)) return;
    const tag = `${selectedId}:${command}`;
    setBusy(tag);
    try {
      await commandRun(selectedId, command);
      setSelectedState(command === 'pause' ? 'Paused' : command === 'resume' ? 'Running' : 'Cancelling');
      await refresh();
      setBanner({ kind: 'info', text: `Comando ${command} inviato.` });
    } catch (err) {
      showError(err);
    } finally {
      setBusy(null);
    }
  }

  async function sendRecover(): Promise<void> {
    if (!selectedId || busy) return;
    setBusy(`${selectedId}:recover`);
    try {
      await recoverRun(selectedId);
      await refresh();
      setBanner({ kind: 'info', text: 'Recover richiesto: il run verrà ripristinato dall\u2019ultimo checkpoint.' });
    } catch (err) {
      showError(err);
    } finally {
      setBusy(null);
    }
  }

  /* ---- render ------------------------------------------------------ */

  const allowed = selectedState ? allowedCommands(selectedState) : new Set<'pause' | 'resume' | 'cancel' | 'recover'>();
  const canAct = (c: 'pause' | 'resume' | 'cancel' | 'recover'): boolean =>
    !!selectedId && allowed.has(c) && busy === null;

  const socketLabel: Record<SocketStatus, string> = {
    connecting: 'connessione…',
    connected: 'live',
    reconnecting: 'riconnessione automatica…',
    offline: 'offline — clicca Refresh o riseleziona il run',
  };

  return <main className="shell">
    <section className="sidebar">
      <h1>EdgePoint Quant Lab</h1>
      <div className="sidebar-actions">
        <button className="primary" onClick={() => void startRun()} disabled={creating}>
          {creating ? 'Creazione…' : 'Nuovo run'}
        </button>
        <button onClick={() => void refresh()}>Refresh</button>
      </div>
      <h2>Runs</h2>
      <div className="runs" aria-busy={creating}>
        {runs.map((run) => (
          <button
            key={run.run_id}
            className={`run-item${selectedId === run.run_id ? ' selected' : ''}`}
            onClick={() => setSelectedId(run.run_id)}
            title={`${run.run_id} · pipeline ${run.pipeline_version}`}
          >
            <span className="run-id">{run.run_id}</span>
            <span className={`state-chip chip-${run.state.toLowerCase()}`}>{run.state}</span>
            <span className="run-time">{formatDateTime(run.updated_at)}</span>
          </button>
        ))}
        {runs.length === 0 && <p className="empty-hint">Nessun run ancora.<br />Creane uno per iniziare.</p>}
        {runsError && <p className="empty-hint error-text">Lista non disponibile. <button onClick={() => void refresh()}>Riprova</button></p>}
      </div>
    </section>

    <section className="content">
      {banner && (
        <div className={`banner ${banner.kind}`} role={banner.kind === 'error' ? 'alert' : 'status'}>
          <span>{banner.text}</span>
          <button className="banner-close" onClick={() => setBanner(null)} aria-label="Chiudi avviso">×</button>
        </div>
      )}

      <div className="toolbar">
        <div className="toolbar-meta">
          <div>Run: <strong title={selectedId ?? ''}>{selectedId ? shortId(selectedId) : 'nessuno'}</strong></div>
          <div>
            Stato: <strong>{selectedState ?? '—'}</strong>
            {' · '}
            <span className={`socket socket-${socketStatus}`}>{selectedId ? socketLabel[socketStatus] : 'inattivo'}</span>
          </div>
        </div>
        {selectedId && (
          <div className="commands" role="group" aria-label="Comandi run">
            <button onClick={() => void sendCommand('pause')} disabled={!canAct('pause')} title={allowed.has('pause') ? 'Metti in pausa il run' : 'Non disponibile nello stato attuale'}>Pause</button>
            <button onClick={() => void sendCommand('resume')} disabled={!canAct('resume')} title={allowed.has('resume') ? 'Riprendi il run' : 'Non disponibile nello stato attuale'}>Resume</button>
            <button onClick={() => void sendRecover()} disabled={!canAct('recover')} title={allowed.has('recover') ? 'Ripristina dall’ultimo checkpoint' : 'Disponibile solo per run falliti o annullati'}>Recover</button>
            <button className="danger" onClick={() => void sendCommand('cancel')} disabled={!canAct('cancel')} title={allowed.has('cancel') ? 'Annulla il run' : 'Non disponibile nello stato attuale'}>Cancel</button>
          </div>
        )}
      </div>

      <section className="panels" aria-busy={loadingPanels}>
        <article className="panel"><h2>Ranking</h2><pre>{ranking ? JSON.stringify(ranking, null, 2) : 'Nessun ranking caricato'}</pre></article>
        <article className="panel">
          <h2>Artifacts</h2>
          {artifacts.length > 0
            ? <ul>{artifacts.map((a) => <li key={a.path}><a href={`/api/runs/${selectedId}/artifacts/${encodeURIComponent(a.path).replace(/%2F/g, '/')}`}>{a.path}</a> <span>{a.bytes} B</span></li>)}</ul>
            : <p className="panel-empty">Nessun artifact</p>}
        </article>
        <article className="panel"><h2>Metrics</h2><pre>{metrics.length ? JSON.stringify(metrics.slice(0, 20), null, 2) : 'Nessuna metrica caricata'}</pre></article>
        <article className="panel"><h2>Trades</h2><pre>{trades.length ? JSON.stringify(trades.slice(0, 20), null, 2) : 'Nessun trade caricato'}</pre></article>
        <article className="panel"><h2>Equity</h2><pre>{equity.length ? JSON.stringify(equity.slice(0, 20), null, 2) : 'Nessuna equity caricata'}</pre></article>
        <article className="panel"><h2>Search State</h2><pre>{searchState ? JSON.stringify(searchState, null, 2) : 'Stato di ricerca non caricato'}</pre></article>
        <article className="panel"><h2>Validation</h2><pre>{validation ? JSON.stringify(validation, null, 2) : 'Validazione non caricata'}</pre></article>
      </section>

      <h2 className="events-title">Progresso realtime</h2>
      <div className="events" aria-live="polite">
        {events.length === 0 && <p className="empty-hint">{selectedId ? 'In attesa dei primi eventi…' : 'Seleziona un run per vedere il progresso.'}</p>}
        {events.map((event) => (
          <article key={event.key} className={`event ${event.status.toLowerCase()}`}>
            <header>
              <strong>{event.stage}</strong>
              <span className={`state-chip chip-${event.status.toLowerCase()}`}>{event.status}</span>
              <span>{event.percent != null ? `${event.percent.toFixed(1)}%` : '—'}</span>
            </header>
            {(event.best_score_so_far != null || event.total != null || event.worker_id) && (
              <p className="event-meta">
                {event.best_score_so_far != null && <>best score: <strong>{event.best_score_so_far}</strong> · </>}
                {event.total != null && <>progresso: {event.current}/{event.total} · </>}
                {event.worker_id && <>worker {event.worker_id} · </>}
                <time dateTime={safeIso(event.timestamp)}>{formatClock(event.timestamp)}</time>
              </p>
            )}
            <p>{event.message}</p>
            {event.error && <pre className="event-error">{event.error.code}: {event.error.message}</pre>}
          </article>
        ))}
      </div>
    </section>
  </main>;
}

function safeIso(timestamp: number): string {
  try { return new Date(timestamp).toISOString(); } catch { return ''; }
}

createRoot(document.getElementById('root')!).render(<App />);

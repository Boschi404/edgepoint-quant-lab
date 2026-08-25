import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { artifactUrl, commandRun, createRun, getEquity, getMetrics, getRanking, getSearchState, getTrades, getValidation, listArtifacts, listRuns, openProgressSocket, recoverRun } from './api';
import type { ArtifactEntry, ProgressEvent, RunSummary } from './types';
import './styles.css';

function App() {
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const [events, setEvents] = useState<ProgressEvent[]>([]);
  const [socketState, setSocketState] = useState('disconnected');
  const [ranking, setRanking] = useState<unknown>(null);
  const [artifacts, setArtifacts] = useState<ArtifactEntry[]>([]);
  const [metrics, setMetrics] = useState<unknown[]>([]);
  const [trades, setTrades] = useState<unknown[]>([]);
  const [equity, setEquity] = useState<unknown[]>([]);
  const [searchState, setSearchState] = useState<unknown>(null);
  const [validation, setValidation] = useState<unknown>(null);
  const socket = useRef<WebSocket | null>(null);

  async function refresh() {
    setRuns(await listRuns());
  }

  async function startRun() {
    const run = await createRun();
    await refresh();
    setSelectedRun(run.run_id);
  }

  useEffect(() => { refresh().catch(console.error); }, []);

  useEffect(() => {
    socket.current?.close();
    setEvents([]);
    setRanking(null);
    setArtifacts([]);
    setMetrics([]);
    setTrades([]);
    setEquity([]);
    setSearchState(null);
    setValidation(null);
    if (!selectedRun) return;
    socket.current = openProgressSocket(selectedRun, (event) => {
      setEvents((prev) => [event, ...prev].slice(0, 200));
      if (event.status === 'Completed') {
        getRanking(selectedRun).then(setRanking).catch(console.error);
        listArtifacts(selectedRun).then(setArtifacts).catch(console.error);
        getMetrics(selectedRun).then(setMetrics).catch(console.error);
        getTrades(selectedRun).then(setTrades).catch(console.error);
        getEquity(selectedRun).then(setEquity).catch(console.error);
        getSearchState(selectedRun).then(setSearchState).catch(console.error);
        getValidation(selectedRun).then(setValidation).catch(console.error);
      }
    }, setSocketState);
    return () => socket.current?.close();
  }, [selectedRun]);

  return <main className="shell">
    <section className="sidebar">
      <h1>Quant System</h1>
      <button onClick={startRun}>New run</button>
      <button onClick={refresh}>Refresh</button>
      <h2>Runs</h2>
      <div className="runs">
        {runs.map((run) => <button key={run.run_id} className={selectedRun === run.run_id ? 'selected' : ''} onClick={() => setSelectedRun(run.run_id)}>
          <strong>{run.run_id}</strong>
          <span>{run.state}</span>
        </button>)}
      </div>
    </section>

    <section className="content">
      <div className="toolbar">
        <div>Selected: <strong>{selectedRun ?? 'none'}</strong></div>
        <div>WebSocket: <strong>{socketState}</strong></div>
        {selectedRun && <div className="commands">
          <button onClick={() => commandRun(selectedRun, 'pause')}>Pause</button>
          <button onClick={() => commandRun(selectedRun, 'resume')}>Resume</button>
          <button onClick={() => recoverRun(selectedRun).then(refresh)}>Recover</button>
          <button onClick={() => commandRun(selectedRun, 'cancel')}>Cancel</button>
          <button onClick={() => getRanking(selectedRun).then(setRanking)}>Ranking</button>
          <button onClick={() => listArtifacts(selectedRun).then(setArtifacts)}>Artifacts</button>
          <button onClick={() => getMetrics(selectedRun).then(setMetrics)}>Metrics</button>
          <button onClick={() => getTrades(selectedRun).then(setTrades)}>Trades</button>
          <button onClick={() => getEquity(selectedRun).then(setEquity)}>Equity</button>
          <button onClick={() => getSearchState(selectedRun).then(setSearchState)}>Search</button>
          <button onClick={() => getValidation(selectedRun).then(setValidation)}>Validation</button>
        </div>}
      </div>
      <section className="panels">
        <article className="panel">
          <h2>Ranking</h2>
          <pre>{ranking ? JSON.stringify(ranking, null, 2) : 'No ranking loaded yet'}</pre>
        </article>
        <article className="panel">
          <h2>Artifacts</h2>
          {selectedRun && artifacts.length > 0 ? <ul>{artifacts.map((a) => <li key={a.path}><a href={artifactUrl(selectedRun, a.path)}>{a.path}</a> <span>{a.bytes} bytes</span></li>)}</ul> : <p>No artifacts loaded yet</p>}
        </article>
        <article className="panel">
          <h2>Metrics</h2>
          <pre>{metrics.length ? JSON.stringify(metrics.slice(0, 20), null, 2) : 'No metrics loaded yet'}</pre>
        </article>
        <article className="panel">
          <h2>Trades</h2>
          <pre>{trades.length ? JSON.stringify(trades.slice(0, 20), null, 2) : 'No trades loaded yet'}</pre>
        </article>
        <article className="panel">
          <h2>Equity</h2>
          <pre>{equity.length ? JSON.stringify(equity.slice(0, 20), null, 2) : 'No equity loaded yet'}</pre>
        </article>
        <article className="panel">
          <h2>Search State</h2>
          <pre>{searchState ? JSON.stringify(searchState, null, 2) : 'No search state loaded yet'}</pre>
        </article>
        <article className="panel">
          <h2>Validation</h2>
          <pre>{validation ? JSON.stringify(validation, null, 2) : 'No validation loaded yet'}</pre>
        </article>
      </section>
      <h2>Realtime progress</h2>
      <div className="events">
        {events.map((event, idx) => <article key={idx} className={`event ${event.status.toLowerCase()}`}>
          <header><strong>{event.stage}</strong><span>{event.status}</span><span>{event.percent?.toFixed(1) ?? '-'}%</span></header>
          <p>{event.message}</p>
          {event.error && <pre>{event.error.code}: {event.error.message}</pre>}
        </article>)}
      </div>
    </section>
  </main>;
}

createRoot(document.getElementById('root')!).render(<App />);

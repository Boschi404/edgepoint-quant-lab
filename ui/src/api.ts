import type { ArtifactEntry, ProgressEvent, RunSummary, WsEnvelope } from './types';

export async function listRuns(): Promise<RunSummary[]> {
  const res = await fetch('/api/runs');
  if (!res.ok) throw new Error(`listRuns failed: ${res.status}`);
  return res.json();
}

export async function createRun(): Promise<RunSummary> {
  const res = await fetch('/api/runs', { method: 'POST' });
  if (!res.ok) throw new Error(`createRun failed: ${res.status}`);
  return res.json();
}

export async function commandRun(runId: string, command: 'pause' | 'resume' | 'cancel'): Promise<void> {
  const res = await fetch(`/api/runs/${runId}/${command}`, { method: 'POST' });
  if (!res.ok) throw new Error(`${command} failed: ${res.status}`);
}

export function openProgressSocket(runId: string, onEvent: (event: ProgressEvent) => void, onState?: (state: string) => void): WebSocket {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const ws = new WebSocket(`${proto}://${location.host}/api/ws/runs/${runId}`);
  ws.onopen = () => onState?.('connected');
  ws.onclose = () => onState?.('disconnected');
  ws.onerror = () => onState?.('error');
  ws.onmessage = (message) => {
    const envelope = JSON.parse(message.data) as WsEnvelope<ProgressEvent>;
    if (envelope.message_type === 'Progress' || envelope.message_type === 'Snapshot') {
      onEvent(envelope.payload);
    }
  };
  return ws;
}

export async function getRanking(runId: string): Promise<unknown> {
  const res = await fetch(`/api/runs/${runId}/ranking`);
  if (!res.ok) throw new Error(`getRanking failed: ${res.status}`);
  return res.json();
}

export async function listArtifacts(runId: string): Promise<ArtifactEntry[]> {
  const res = await fetch(`/api/runs/${runId}/artifacts`);
  if (!res.ok) throw new Error(`listArtifacts failed: ${res.status}`);
  return res.json();
}

export function artifactUrl(runId: string, path: string): string {
  return `/api/runs/${runId}/artifacts/${encodeURIComponent(path).replace(/%2F/g, '/')}`;
}

export async function getMetrics(runId: string): Promise<unknown[]> {
  const res = await fetch(`/api/runs/${runId}/results/metrics`);
  if (!res.ok) throw new Error(`getMetrics failed: ${res.status}`);
  return res.json();
}

export async function getTrades(runId: string): Promise<unknown[]> {
  const res = await fetch(`/api/runs/${runId}/results/trades`);
  if (!res.ok) throw new Error(`getTrades failed: ${res.status}`);
  return res.json();
}

export async function getEquity(runId: string): Promise<unknown[]> {
  const res = await fetch(`/api/runs/${runId}/results/equity`);
  if (!res.ok) throw new Error(`getEquity failed: ${res.status}`);
  return res.json();
}

export async function recoverRun(runId: string): Promise<RunSummary> {
  const res = await fetch(`/api/runs/${runId}/recover`, { method: 'POST' });
  if (!res.ok) throw new Error(`recoverRun failed: ${res.status}`);
  return res.json();
}

export async function getSearchState(runId: string): Promise<unknown> {
  const res = await fetch(`/api/runs/${runId}/search-state`);
  if (!res.ok) throw new Error(`getSearchState failed: ${res.status}`);
  return res.json();
}

export async function getValidation(runId: string): Promise<unknown> {
  const res = await fetch(`/api/runs/${runId}/validation`);
  if (!res.ok) throw new Error(`getValidation failed: ${res.status}`);
  return res.json();
}

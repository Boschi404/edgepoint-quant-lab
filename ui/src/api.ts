import type { ArtifactEntry, ProgressEvent, RunSummary, WsEnvelope } from './types';

export class ApiError extends Error {
  constructor(
    public readonly op: string,
    public readonly status: number,
  ) {
    super(`${op} failed: ${status}`);
  }
}

async function request<T>(op: string, url: string, init?: RequestInit): Promise<T> {
  let res: Response;
  try {
    res = await fetch(url, init);
  } catch {
    throw new ApiError(op, 0);
  }
  if (!res.ok) throw new ApiError(op, res.status);
  return res.json() as Promise<T>;
}

export function listRuns(): Promise<RunSummary[]> {
  return request('list runs', '/api/runs');
}

export function createRun(): Promise<RunSummary> {
  return request('create run', '/api/runs', { method: 'POST' });
}

export function getRun(runId: string): Promise<RunSummary> {
  return request('load run', `/api/runs/${runId}`);
}

export function commandRun(runId: string, command: 'pause' | 'resume' | 'cancel'): Promise<void> {
  return request(`${command} run`, `/api/runs/${runId}/${command}`, { method: 'POST' }).then(() => undefined);
}

export function recoverRun(runId: string): Promise<RunSummary> {
  return request('recover run', `/api/runs/${runId}/recover`, { method: 'POST' });
}

export function getRanking(runId: string): Promise<unknown> {
  return request('load ranking', `/api/runs/${runId}/ranking`);
}

export function listArtifacts(runId: string): Promise<ArtifactEntry[]> {
  return request('load artifacts', `/api/runs/${runId}/artifacts`);
}

export function artifactUrl(runId: string, path: string): string {
  return `/api/runs/${runId}/artifacts/${encodeURIComponent(path).replace(/%2F/g, '/')}`;
}

export function getMetrics(runId: string): Promise<unknown[]> {
  return request('load metrics', `/api/runs/${runId}/results/metrics`);
}

export function getTrades(runId: string): Promise<unknown[]> {
  return request('load trades', `/api/runs/${runId}/results/trades`);
}

export function getEquity(runId: string): Promise<unknown[]> {
  return request('load equity', `/api/runs/${runId}/results/equity`);
}

export function getSearchState(runId: string): Promise<unknown> {
  return request('load search state', `/api/runs/${runId}/search-state`);
}

export function getValidation(runId: string): Promise<unknown> {
  return request('load validation', `/api/runs/${runId}/validation`);
}

export type SocketStatus = 'connecting' | 'connected' | 'reconnecting' | 'offline';

export interface ProgressConnection {
  close(): void;
}

interface ProgressHandlers {
  onEvent(event: ProgressEvent): void;
  onState(status: SocketStatus): void;
}

/**
 * Opens the progress WebSocket for a run and keeps it alive: on abnormal
 * close it retries with exponential backoff (1s..15s, gives up after 6 tries
 * -> 'offline'), so a dropped connection is visible instead of silently
 * freezing the feed. Close the returned handle on selection change/unmount.
 */
export function connectProgress(runId: string, handlers: ProgressHandlers): ProgressConnection {
  let closedByUser = false;
  let attempt = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let ws: WebSocket | null = null;

  const scheduleRetry = (): void => {
    if (closedByUser) return;
    if (attempt >= 6) {
      handlers.onState('offline');
      return;
    }
    const delay = Math.min(15000, 1000 * 2 ** attempt);
    attempt += 1;
    handlers.onState('reconnecting');
    timer = setTimeout(open, delay);
  };

  const open = (): void => {
    if (closedByUser) return;
    handlers.onState(attempt === 0 ? 'connecting' : 'reconnecting');
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    try {
      ws = new WebSocket(`${proto}://${location.host}/api/ws/runs/${runId}`);
    } catch {
      scheduleRetry();
      return;
    }
    ws.onopen = () => {
      attempt = 0;
      handlers.onState('connected');
    };
    ws.onmessage = (message: MessageEvent<string>) => {
      let envelope: WsEnvelope<ProgressEvent>;
      try {
        envelope = JSON.parse(message.data) as WsEnvelope<ProgressEvent>;
      } catch {
        return;
      }
      if (
        (envelope.message_type === 'Progress' || envelope.message_type === 'Snapshot')
        && envelope.payload
      ) {
        handlers.onEvent(envelope.payload);
      }
    };
    ws.onclose = () => scheduleRetry();
    ws.onerror = () => {
      /* onclose follows; retry handled there */
    };
  };

  open();
  return {
    close(): void {
      closedByUser = true;
      if (timer !== undefined) clearTimeout(timer);
      ws?.close();
    },
  };
}

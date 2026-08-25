export type RunStatus = 'Pending' | 'Running' | 'Paused' | 'Completed' | 'Failed' | 'Cancelled';

export interface SerializableError {
  code: string;
  category: string;
  message: string;
  retryable: boolean;
  timestamp: number;
}

export interface ProgressEvent {
  schema_version: number;
  run_id: string | { 0: string };
  stage: string;
  status: RunStatus;
  worker_id?: string | null;
  current: number;
  total?: number | null;
  percent?: number | null;
  best_score_so_far?: number | null;
  message: string;
  error?: SerializableError | null;
  timestamp: number;
}

export interface WsEnvelope<T> {
  schema_version: number;
  message_type: string;
  run_id?: string | null;
  sequence: number;
  payload: T;
}

export interface RunSummary {
  run_id: string;
  state: string;
  created_at: number;
  updated_at: number;
  pipeline_version: string;
}

export interface ArtifactEntry {
  path: string;
  bytes: number;
}

/**
 * Scheduled BlueScript runs of a hosted file (CORE-SCRIPT-002): the server keeps
 * a copy of a saved script with a cron-like schedule (`POST
 * /api/sessions/{id}/schedules`) and runs it headless. Only scripts the user
 * picks from their own saved scripts are scheduled — never code embedded in a
 * document.
 */

/** One schedule (mirrors the server's `ScriptScheduleDto`). */
export interface ScriptSchedule {
  id: string;
  name: string;
  /** Five fields: minute hour day-of-month month day-of-week (UTC), e.g. `0 7 * * 1-5`. */
  cron: string;
  enabled: boolean;
  /** ISO date-times. */
  nextRunAt: string | null;
  lastRunAt: string | null;
  /** `succeeded`, `failed`, `skipped` (file open for editing) or `unavailable` (runs switched off). */
  lastStatus: string | null;
  lastError: string | null;
  /** Failed runs in a row (CORE-SCRIPT-004). */
  consecutiveFailures: number;
  /** Why the server disabled the schedule (too many failures). */
  disabledReason: string | null;
}

/** One run in a schedule's history (CORE-SCRIPT-004). */
export interface ScriptRunRecord {
  /** ISO date-time. */
  startedAt: string;
  durationMs: number;
  status: string;
  error: string | null;
  /** Console output (`[level] text` lines, truncated). */
  logs: string | null;
  /** Host version the run saved. */
  version: string | null;
}

export interface ScriptScheduleInput {
  name: string;
  code: string;
  cron: string;
  enabled?: boolean;
}

/** Schedules of the open file for the signed-in user (hosted sessions only). */
export interface ScriptSchedules {
  list(): Promise<ScriptSchedule[]>;
  /** Rejects with `Error('schedule.invalidCron')` for a malformed schedule. */
  create(input: ScriptScheduleInput): Promise<ScriptSchedule>;
  remove(id: string): Promise<void>;
  /** Run history, newest first. */
  runs?(id: string): Promise<ScriptRunRecord[]>;
  /** Enables (resetting the failure count) or disables a schedule. */
  setEnabled?(id: string, enabled: boolean): Promise<ScriptSchedule>;
}

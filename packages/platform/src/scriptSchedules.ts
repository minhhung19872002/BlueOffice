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
  /** `succeeded`, `failed` or `unavailable` (no server-side runner configured). */
  lastStatus: string | null;
  lastError: string | null;
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
}

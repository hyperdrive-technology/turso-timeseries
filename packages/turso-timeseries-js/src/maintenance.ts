import type { WasmDatabase } from './loadExtension.js';

export type MaintenanceOptions = {
  compact?: boolean;
  downsample?: boolean;
  retention?: boolean;
  refreshStats?: boolean;
};

export async function runMaintenance(
  db: WasmDatabase,
  _options: MaintenanceOptions = {},
): Promise<void> {
  await db.exec(
    `INSERT INTO _tts_maintenance_jobs (job_kind, due_at_micros) VALUES ('stats', CAST((julianday('now') - 2440587.5) * 86400000000 AS INTEGER))`,
  );
}

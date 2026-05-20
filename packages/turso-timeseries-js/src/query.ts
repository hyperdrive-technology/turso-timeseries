import type { WasmDatabase } from './loadExtension.js';

export type RollupRow = {
  bucket_ns: number;
  avg_value_real: number | null;
};

export async function queryRollup(
  db: WasmDatabase,
  rollupTable: string,
): Promise<RollupRow[]> {
  const rows = await (db as WasmDatabase & {
    all(sql: string, ...params: unknown[]): Promise<RollupRow[]>;
  }).all(
    `SELECT bucket_ns, avg_value_real FROM _tts_rollups WHERE rollup_table = ? ORDER BY bucket_ns`,
    rollupTable,
  );
  return rows;
}

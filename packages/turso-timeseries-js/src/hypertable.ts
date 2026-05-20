import type { WasmDatabase } from './loadExtension.js';

export async function createHypertable(
  db: WasmDatabase,
  table: string,
  chunkIntervalMicros: number,
): Promise<void> {
  await db.exec(
    `INSERT INTO _tts_hypertables (table_name, time_column, chunk_interval_ns, storage_layout) \
     VALUES ('${table}', 'time', ${chunkIntervalMicros * 1000}, 'segment_v1') \
     ON CONFLICT(table_name) DO UPDATE SET chunk_interval_ns = excluded.chunk_interval_ns`,
  );
}

import { connect } from '@tursodatabase/database-wasm/vite';
import { loadTimeseriesExtension, migrations } from './tursoTimeseriesExtension';

export type E2EResult =
  | { ok: true; rows: unknown[]; bucket: string; extensionLoaded: number }
  | { ok: false; error: string };

export async function runTursoTimeseriesBrowserE2E(): Promise<E2EResult> {
  try {
    const db = await connect(':memory:');
    const extension = await loadTimeseriesExtension(db);

    for (const sql of migrations) {
      const statement = await db.prepare(sql);
      await statement.run();
    }

    const insertSeries = await db.prepare(
      'INSERT OR IGNORE INTO _tts_series (metric_name, tags_json) VALUES (?, ?)',
    );
    await insertSeries.run(['temperature', '{"device":"browser"}']);

    const insertSample = await db.prepare(
      `INSERT INTO _tts_samples (series_id, ts_ns, value_real, quality)
         SELECT series_id, ?, ?, ?
         FROM _tts_series
         WHERE metric_name = ? AND tags_json = ?`,
    );
    await insertSample.run([1778544000000000000n, 23.4, 0, 'temperature', '{"device":"browser"}']);

    const selectSamples = await db.prepare(
      `SELECT s.metric_name, s.tags_json, p.ts_ns, p.value_real, p.quality
         FROM _tts_samples p
         JOIN _tts_series s ON s.series_id = p.series_id
         ORDER BY p.ts_ns`,
    );
    const rows = await selectSamples.all();
    const bucket = await extension.timeBucketNs(1778544000000000000n, 60000000000n);

    return {
      ok: true,
      rows,
      bucket: bucket.toString(),
      extensionLoaded: extension.extensionLoaded,
    };
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.stack ?? error.message : String(error),
    };
  }
}

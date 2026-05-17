import { expect, test } from 'vitest';
import { runTursoTimeseriesBrowserE2E } from '../src/runTursoTimeseriesBrowserE2E';
import { createSandboxDatabase, renderSandbox, runSandboxQuery } from '../src/sandbox';

test('browser loads turso-wasm and timeseries wasm module then round trips samples', async () => {
  const result = await runTursoTimeseriesBrowserE2E();

  expect(result.ok, result.ok ? undefined : result.error).toBe(true);
  if (!result.ok) {
    return;
  }

  expect(result.extensionLoaded).toBe(1);
  expect(result.bucket).toBe('1778544000000000000');
  expect(result.rows).toHaveLength(1);
  expect(result.rows[0]).toMatchObject({
    metric_name: 'temperature',
    tags_json: '{"device":"browser"}',
    value_real: 23.4,
    quality: 0,
  });
});

test('sandbox runs read and write SQL directly against the browser Turso database', async () => {
  const db = await createSandboxDatabase();

  const extensionInfo = await runSandboxQuery(db, 'SELECT * FROM _tts_extension_info');
  expect(extensionInfo.ok, extensionInfo.ok ? undefined : extensionInfo.error).toBe(true);
  if (!extensionInfo.ok || extensionInfo.kind !== 'rows') {
    return;
  }
  expect(extensionInfo.rows).toEqual([
    {
      singleton: 1,
      extension_name: 'turso-timeseries-wasm-extension',
      wasm_loaded: 1,
      bucket_width_ns: 60000000000,
    },
  ]);

  const firstRead = await runSandboxQuery(db, 'SELECT COUNT(*) AS count FROM _tts_samples');
  expect(firstRead.ok, firstRead.ok ? undefined : firstRead.error).toBe(true);
  if (!firstRead.ok || firstRead.kind !== 'rows') {
    return;
  }
  expect(firstRead.rows).toEqual([{ count: 1 }]);

  const write = await runSandboxQuery(
    db,
    `INSERT INTO _tts_samples (series_id, ts_ns, value_real, quality)
     SELECT series_id, 1778544030000000000, 24.1, 0
     FROM _tts_series
     WHERE metric_name = 'temperature' AND tags_json = '{"device":"browser"}'`,
  );
  expect(write.ok, write.ok ? undefined : write.error).toBe(true);
  expect(write.ok && write.kind).toBe('run');

  const secondRead = await runSandboxQuery(
    db,
    'SELECT ts_ns, value_real FROM _tts_samples ORDER BY ts_ns',
  );
  expect(secondRead.ok, secondRead.ok ? undefined : secondRead.error).toBe(true);
  if (!secondRead.ok || secondRead.kind !== 'rows') {
    return;
  }
  expect(secondRead.rows).toEqual([
    { ts_ns: 1778544000000000000, value_real: 23.4 },
    { ts_ns: 1778544030000000000, value_real: 24.1 },
  ]);

  const bucketRead = await runSandboxQuery(
    db,
    `SELECT bucket_ns, COUNT(*) AS sample_count, AVG(value_real) AS avg_value
     FROM _tts_sample_buckets
     GROUP BY bucket_ns
     ORDER BY bucket_ns`,
  );
  expect(bucketRead.ok, bucketRead.ok ? undefined : bucketRead.error).toBe(true);
  if (!bucketRead.ok || bucketRead.kind !== 'rows') {
    return;
  }
  expect(bucketRead.rows).toEqual([{ bucket_ns: 1778544000000000000, sample_count: 2, avg_value: 23.75 }]);
});

test('sandbox registers columnar hypertables with supported browser Turso SQL', async () => {
  const db = await createSandboxDatabase();

  const create = await runSandboxQuery(
    db,
    `CREATE TABLE IF NOT EXISTS browser_samples (
       ts_ns INTEGER NOT NULL,
       value_real REAL,
       quality INTEGER NOT NULL DEFAULT 0,
       PRIMARY KEY (ts_ns)
     );
     INSERT INTO _tts_hypertables (table_name, time_column, chunk_interval_ns, storage_layout)
     VALUES ('browser_samples', 'ts_ns', 60000000000, 'columnar')
     ON CONFLICT(table_name) DO UPDATE SET
       chunk_interval_ns = excluded.chunk_interval_ns,
       storage_layout = 'columnar';`,
  );

  expect(create.ok, create.ok ? undefined : create.error).toBe(true);
  expect(create.ok && create.kind).toBe('exec');

  const table = await selectRows(
    db,
    `SELECT name
     FROM sqlite_schema
     WHERE type = 'table' AND name = 'browser_samples'`,
  );
  expect(table).toEqual([{ name: 'browser_samples' }]);

  const catalog = await selectRows(
    db,
    `SELECT table_name, time_column, chunk_interval_ns, storage_layout
     FROM _tts_hypertables
     WHERE table_name = 'browser_samples'`,
  );
  expect(catalog).toEqual([
    {
      table_name: 'browser_samples',
      time_column: 'ts_ns',
      chunk_interval_ns: 60000000000,
      storage_layout: 'columnar',
    },
  ]);

  const insert = await runSandboxQuery(
    db,
    `INSERT INTO browser_samples (ts_ns, value_real, quality)
     VALUES (1778544000000000000, 23.4, 0)`,
  );
  expect(insert.ok, insert.ok ? undefined : insert.error).toBe(true);

  const samples = await selectRows(
    db,
    `SELECT ts_ns, value_real, quality
     FROM browser_samples
     ORDER BY ts_ns`,
  );
  expect(samples).toEqual([{ ts_ns: 1778544000000000000, value_real: 23.4, quality: 0 }]);

  const unsupported = await runSandboxQuery(
    db,
    `CREATE TABLE unsupported_samples (
       ts_ns INTEGER NOT NULL,
       value_real REAL
     ) WITH (
       tsdb.hypertable
     );`,
  );
  expect(unsupported.ok).toBe(false);
});

test('sandbox rollups downsample more than five minutes of columnar browser Turso samples', async () => {
  const db = await createSandboxDatabase();
  const baseTsNs = 1778544000000000000n;
  const secondNs = 1_000_000_000n;

  await db.run('DELETE FROM _tts_samples');

  for (let second = 0; second < 360; second += 1) {
    await db.run(
      `INSERT INTO _tts_samples (series_id, ts_ns, value_real, quality)
       SELECT series_id, ?, ?, 0
       FROM _tts_series
       WHERE metric_name = 'temperature' AND tags_json = '{"device":"browser"}'`,
      baseTsNs + BigInt(second) * secondNs,
      second,
    );
  }

  const rollups = await selectRows(
    db,
    `SELECT
       CAST((c.chunk_start_ns - ${baseTsNs}) / 60000000000 AS INTEGER) AS bucket_index,
       seg.row_count AS sample_count,
       seg.min_value_real AS min_value,
       seg.max_value_real AS max_value,
       seg.sum_value_real / seg.row_count AS avg_value
     FROM _tts_segments seg
     JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id
     ORDER BY c.chunk_start_ns`,
  );

  expect(rollups).toEqual([
    { bucket_index: 0, sample_count: 60, min_value: 0, max_value: 59, avg_value: 29.5 },
    { bucket_index: 1, sample_count: 60, min_value: 60, max_value: 119, avg_value: 89.5 },
    { bucket_index: 2, sample_count: 60, min_value: 120, max_value: 179, avg_value: 149.5 },
    { bucket_index: 3, sample_count: 60, min_value: 180, max_value: 239, avg_value: 209.5 },
    { bucket_index: 4, sample_count: 60, min_value: 240, max_value: 299, avg_value: 269.5 },
    { bucket_index: 5, sample_count: 60, min_value: 300, max_value: 359, avg_value: 329.5 },
  ]);

  const downsampled = await selectRows(
    db,
    `SELECT
       CAST((c.chunk_start_ns - ${baseTsNs}) / 60000000000 AS INTEGER) AS minute,
       seg.sum_value_real / seg.row_count AS value_real
     FROM _tts_segments seg
     JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id
     ORDER BY c.chunk_start_ns`,
  );

  expect(downsampled).toEqual([
    { minute: 0, value_real: 29.5 },
    { minute: 1, value_real: 89.5 },
    { minute: 2, value_real: 149.5 },
    { minute: 3, value_real: 209.5 },
    { minute: 4, value_real: 269.5 },
    { minute: 5, value_real: 329.5 },
  ]);

  const columns = await selectRows(
    db,
     `SELECT column_name, value_type, encoding, length(data_blob) AS bytes
     FROM _tts_segment_columns
     WHERE segment_id = (SELECT MIN(segment_id) FROM _tts_segments)
     ORDER BY column_name`,
  );

  expect(columns).toEqual([
    { column_name: 'quality', value_type: 'integer', encoding: 'i32-le', bytes: 240 },
    { column_name: 'ts_ns', value_type: 'integer', encoding: 'i64-le', bytes: 480 },
    { column_name: 'value_real', value_type: 'real', encoding: 'f64-le', bytes: 480 },
  ]);

  const materialized = await selectRows(
    db,
    `SELECT CAST((bucket_ns - ${baseTsNs}) / 60000000000 AS INTEGER) AS minute,
       sample_count, min_value_real, max_value_real, avg_value_real
     FROM _tts_rollups
     WHERE rollup_table = 'samples_1m'
     ORDER BY bucket_ns`,
  );

  expect(materialized).toEqual([
    { minute: 0, sample_count: 60, min_value_real: 0, max_value_real: 59, avg_value_real: 29.5 },
    { minute: 1, sample_count: 60, min_value_real: 60, max_value_real: 119, avg_value_real: 89.5 },
    { minute: 2, sample_count: 60, min_value_real: 120, max_value_real: 179, avg_value_real: 149.5 },
    { minute: 3, sample_count: 60, min_value_real: 180, max_value_real: 239, avg_value_real: 209.5 },
    { minute: 4, sample_count: 60, min_value_real: 240, max_value_real: 299, avg_value_real: 269.5 },
    { minute: 5, sample_count: 60, min_value_real: 300, max_value_real: 359, avg_value_real: 329.5 },
  ]);
});

test('sandbox page renders a query editor and initial result table', async () => {
  const root = document.createElement('div');
  document.body.append(root);

  try {
    renderSandbox(root);

    const editor = root.querySelector<HTMLTextAreaElement>('#query-editor');
    const status = root.querySelector<HTMLOutputElement>('[data-role="status"]');
    const result = root.querySelector<HTMLElement>('[data-role="result"]');

    expect(editor).not.toBeNull();
    expect(status).not.toBeNull();
    expect(result).not.toBeNull();
    expect(editor?.value).toContain('_tts_sample_buckets');

    await waitFor(() => status?.value === 'ready' && result?.querySelectorAll('tbody tr').length === 1);

    expect(result?.textContent).toContain('temperature');
    expect(result?.textContent).toContain('23.4');
  } finally {
    root.remove();
  }
});

async function waitFor(predicate: () => boolean): Promise<void> {
  const start = performance.now();
  while (!predicate()) {
    if (performance.now() - start > 5000) {
      throw new Error('timed out waiting for sandbox page');
    }
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
}

async function selectRows(
  db: Awaited<ReturnType<typeof createSandboxDatabase>>,
  sql: string,
): Promise<Record<string, unknown>[]> {
  const result = await runSandboxQuery(db, sql);
  expect(result.ok, result.ok ? undefined : result.error).toBe(true);
  expect(result.ok && result.kind).toBe('rows');
  if (!result.ok || result.kind !== 'rows') {
    return [];
  }

  return result.rows;
}

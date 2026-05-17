import { connect, type Database } from '@tursodatabase/database-wasm/vite';
import { loadTimeseriesExtension, migrations, type TimeseriesExtension } from './tursoTimeseriesExtension';

type JsonRow = Record<string, unknown>;
const defaultBucketWidthNs = 60_000_000_000n;
const extensionsByDatabase = new WeakMap<Database, TimeseriesExtension>();

export type SandboxQueryResult =
  | {
      ok: true;
      kind: 'rows';
      columns: string[];
      rows: JsonRow[];
      elapsedMs: number;
    }
  | {
      ok: true;
      kind: 'run';
      changes: number;
      lastInsertRowid: number;
      elapsedMs: number;
    }
  | {
      ok: true;
      kind: 'exec';
      elapsedMs: number;
    }
  | {
      ok: false;
      error: string;
      elapsedMs: number;
    };

export const sandboxQueries = {
  extension: `SELECT extension_name, wasm_loaded, bucket_width_ns
FROM _tts_extension_info;`,
  samples: `SELECT s.metric_name, s.tags_json, p.ts_ns, p.value_real, p.quality
FROM _tts_samples p
JOIN _tts_series s ON s.series_id = p.series_id
ORDER BY p.ts_ns;`,
  buckets: `SELECT metric_name, tags_json, bucket_ns, COUNT(*) AS sample_count, AVG(value_real) AS avg_value
FROM _tts_sample_buckets
GROUP BY metric_name, tags_json, bucket_ns
ORDER BY bucket_ns;`,
  columnarSegments: `SELECT s.metric_name, s.tags_json, c.chunk_start_ns, seg.row_count,
       seg.min_value_real, seg.max_value_real,
       seg.sum_value_real / seg.row_count AS avg_value_real
FROM _tts_segments seg
JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id
JOIN _tts_series s ON s.series_id = seg.series_id
ORDER BY c.chunk_start_ns;`,
  columnarColumns: `SELECT seg.segment_id, col.column_name, col.value_type, col.encoding,
       length(col.data_blob) AS bytes
FROM _tts_segment_columns col
JOIN _tts_segments seg ON seg.segment_id = col.segment_id
ORDER BY seg.segment_id, col.column_name;`,
  materializedRollups: `SELECT rollup_table, metric_name, tags_json, bucket_ns, sample_count,
       min_value_real, max_value_real, avg_value_real
FROM _tts_rollups
ORDER BY rollup_table, metric_name, tags_json, bucket_ns;`,
  hypertable: `CREATE TABLE IF NOT EXISTS browser_samples (
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
  bucketedSamples: `SELECT metric_name, tags_json, ts_ns, bucket_ns, value_real, quality
FROM _tts_sample_buckets
ORDER BY ts_ns;`,
  series: `SELECT series_id, metric_name, tags_json, created_at
FROM _tts_series
ORDER BY series_id;`,
  schema: `SELECT name, type, sql
FROM sqlite_schema
WHERE name LIKE '_tts_%'
ORDER BY type, name;`,
  insert: `INSERT INTO _tts_samples (series_id, ts_ns, value_real, quality)
SELECT series_id, 1778544030000000000, 24.1, 0
FROM _tts_series
WHERE metric_name = 'temperature' AND tags_json = '{"device":"browser"}';`,
};

export async function createSandboxDatabase(): Promise<Database> {
  const db = await connect(':memory:');
  extensionsByDatabase.set(db, await loadTimeseriesExtension(db));
  await resetSandboxDatabase(db);
  return db;
}

export async function resetSandboxDatabase(db: Database): Promise<void> {
  await db.exec(`
    DROP TABLE IF EXISTS _tts_sample_buckets;
    DROP TABLE IF EXISTS _tts_extension_info;
    DROP TABLE IF EXISTS _tts_rollups;
    DROP TABLE IF EXISTS _tts_segment_columns;
    DROP TABLE IF EXISTS _tts_segments;
    DROP TABLE IF EXISTS _tts_chunks;
    DROP TABLE IF EXISTS _tts_hypertable_rows;
    DROP TABLE IF EXISTS _tts_hypertables;
    DROP TABLE IF EXISTS _tts_jobs;
    DROP TABLE IF EXISTS _tts_invalidations;
    DROP TABLE IF EXISTS _tts_rollup_watermarks;
    DROP TABLE IF EXISTS _tts_rollup_policies;
    DROP TABLE IF EXISTS _tts_retention_policies;
    DROP TABLE IF EXISTS _tts_samples;
    DROP TABLE IF EXISTS _tts_series;
    DROP TABLE IF EXISTS _tts_schema_version;
  `);

  for (const sql of migrations) {
    await db.exec(sql);
  }

  await seedSandboxDatabase(db);
  await refreshExtensionTables(db);
}

export async function seedSandboxDatabase(db: Database): Promise<void> {
  await db.run(
    'INSERT OR IGNORE INTO _tts_series (metric_name, tags_json) VALUES (?, ?)',
    'temperature',
    '{"device":"browser"}',
  );
  await db.run(
    `INSERT OR REPLACE INTO _tts_samples (series_id, ts_ns, value_real, quality)
     SELECT series_id, ?, ?, ?
     FROM _tts_series
     WHERE metric_name = ? AND tags_json = ?`,
    1778544000000000000n,
    23.4,
    0,
    'temperature',
    '{"device":"browser"}',
  );
}

export async function runSandboxQuery(db: Database, sql: string): Promise<SandboxQueryResult> {
  const start = performance.now();
  const trimmed = sql.trim();

  if (!trimmed) {
    return {
      ok: false,
      error: 'Query is empty.',
      elapsedMs: elapsedSince(start),
    };
  }

  try {
    await refreshExtensionTables(db);
    const statement = await db.prepare(trimmed);
    try {
      if (statement.reader) {
        const rows = normalizeRows(await statement.all());
        return {
          ok: true,
          kind: 'rows',
          columns: columnNames(statement, rows),
          rows,
          elapsedMs: elapsedSince(start),
        };
      }

      const result = await statement.run();
      await refreshExtensionTables(db);
      return {
        ok: true,
        kind: 'run',
        changes: result.changes,
        lastInsertRowid: result.lastInsertRowid,
        elapsedMs: elapsedSince(start),
      };
    } finally {
      statement.close();
    }
  } catch (error) {
    if (trimmed.includes(';')) {
      try {
        await db.exec(trimmed);
        await refreshExtensionTables(db);
        return {
          ok: true,
          kind: 'exec',
          elapsedMs: elapsedSince(start),
        };
      } catch (execError) {
        return {
          ok: false,
          error: errorMessage(execError),
          elapsedMs: elapsedSince(start),
        };
      }
    }

    return {
      ok: false,
      error: errorMessage(error),
      elapsedMs: elapsedSince(start),
    };
  }
}

export function renderSandbox(root: HTMLElement): void {
  root.innerHTML = `
    <section class="sandbox-shell">
      <header class="sandbox-header">
        <div>
          <p class="eyebrow">turso-timeseries</p>
          <h1>Turso WASM Sandbox</h1>
        </div>
        <output class="status" data-role="status">initializing</output>
      </header>
      <div class="toolbar" aria-label="Query presets">
        <button type="button" data-query="extension">Extension</button>
        <button type="button" data-query="samples">Samples</button>
        <button type="button" data-query="buckets">Buckets</button>
        <button type="button" data-query="columnarSegments">Segments</button>
        <button type="button" data-query="columnarColumns">Columns</button>
        <button type="button" data-query="materializedRollups">Rollups</button>
        <button type="button" data-query="hypertable">Hypertable</button>
        <button type="button" data-query="bucketedSamples">Bucketed</button>
        <button type="button" data-query="series">Series</button>
        <button type="button" data-query="schema">Schema</button>
        <button type="button" data-query="insert">Insert</button>
      </div>
      <label class="editor-label" for="query-editor">SQL</label>
      <textarea id="query-editor" spellcheck="false"></textarea>
      <div class="actions">
        <button type="button" class="primary" data-action="run">Run</button>
        <button type="button" data-action="reset">Reset DB</button>
      </div>
      <section class="result-panel" aria-live="polite">
        <div class="result-meta" data-role="meta"></div>
        <div class="result-table" data-role="result"></div>
      </section>
    </section>
  `;

  const editor = mustFind<HTMLTextAreaElement>(root, '#query-editor');
  const status = mustFind<HTMLOutputElement>(root, '[data-role="status"]');
  const meta = mustFind<HTMLElement>(root, '[data-role="meta"]');
  const resultNode = mustFind<HTMLElement>(root, '[data-role="result"]');
  const runButton = mustFind<HTMLButtonElement>(root, '[data-action="run"]');
  const resetButton = mustFind<HTMLButtonElement>(root, '[data-action="reset"]');

  let dbPromise = createSandboxDatabase();
  editor.value = sandboxQueries.buckets;

  dbPromise
    .then(() => {
      status.value = 'ready';
      return executeCurrentQuery();
    })
    .catch((error) => {
      status.value = 'failed';
      renderError(resultNode, meta, errorMessage(error), 0);
    });

  root.querySelectorAll<HTMLButtonElement>('[data-query]').forEach((button) => {
    button.addEventListener('click', () => {
      const queryName = button.dataset.query as keyof typeof sandboxQueries;
      editor.value = sandboxQueries[queryName];
      editor.focus();
    });
  });

  runButton.addEventListener('click', () => {
    void executeCurrentQuery();
  });

  resetButton.addEventListener('click', async () => {
    status.value = 'resetting';
    try {
      const db = await dbPromise;
      await resetSandboxDatabase(db);
      status.value = 'ready';
      await executeCurrentQuery();
    } catch (error) {
      status.value = 'failed';
      renderError(resultNode, meta, errorMessage(error), 0);
    }
  });

  editor.addEventListener('keydown', (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
      event.preventDefault();
      void executeCurrentQuery();
    }
  });

  async function executeCurrentQuery(): Promise<void> {
    status.value = 'running';
    runButton.disabled = true;
    resetButton.disabled = true;

    try {
      const db = await dbPromise;
      const result = await runSandboxQuery(db, editor.value);
      if (result.ok) {
        status.value = 'ready';
        renderResult(resultNode, meta, result);
      } else {
        status.value = 'error';
        renderError(resultNode, meta, result.error, result.elapsedMs);
      }
    } catch (error) {
      status.value = 'error';
      renderError(resultNode, meta, errorMessage(error), 0);
      dbPromise = createSandboxDatabase();
    } finally {
      runButton.disabled = false;
      resetButton.disabled = false;
    }
  }
}

async function refreshExtensionTables(db: Database): Promise<void> {
  const extension = await extensionForDatabase(db);

  try {
    await db.exec(`
      CREATE TABLE IF NOT EXISTS _tts_extension_info (
        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
        extension_name TEXT NOT NULL,
        wasm_loaded INTEGER NOT NULL,
        bucket_width_ns INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS _tts_sample_buckets (
        series_id INTEGER NOT NULL,
        metric_name TEXT NOT NULL,
        tags_json TEXT,
        ts_ns INTEGER NOT NULL,
        bucket_width_ns INTEGER NOT NULL,
        bucket_ns INTEGER NOT NULL,
        value_real REAL,
        quality INTEGER NOT NULL,
        PRIMARY KEY (series_id, ts_ns, bucket_width_ns)
      );
      CREATE INDEX IF NOT EXISTS idx_tts_sample_buckets_bucket
        ON _tts_sample_buckets (bucket_ns, metric_name);
    `);

    await db.run(
      `INSERT OR REPLACE INTO _tts_extension_info
       (singleton, extension_name, wasm_loaded, bucket_width_ns)
       VALUES (1, ?, ?, ?)`,
      'turso-timeseries-wasm-extension',
      extension.extensionLoaded,
      defaultBucketWidthNs,
    );

    const samples = await db.all(
      `SELECT p.series_id, s.metric_name, s.tags_json, p.ts_ns, p.value_real, p.quality
       FROM _tts_samples p
       JOIN _tts_series s ON s.series_id = p.series_id
       ORDER BY p.ts_ns`,
    );

    await db.run('DELETE FROM _tts_sample_buckets');
    for (const sample of normalizeRows(samples)) {
      const tsNs = BigInt(String(sample.ts_ns));
      await db.run(
        `INSERT OR REPLACE INTO _tts_sample_buckets
         (series_id, metric_name, tags_json, ts_ns, bucket_width_ns, bucket_ns, value_real, quality)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        sample.series_id,
        sample.metric_name,
        sample.tags_json,
        tsNs,
        defaultBucketWidthNs,
        await extension.timeBucketNs(tsNs, defaultBucketWidthNs),
        sample.value_real,
        sample.quality,
      );
    }

    await refreshColumnarSegments(db, samples);
    await refreshMaterializedRollups(db);
  } catch {
    // User-entered DDL can temporarily remove backing tables. The next reset or
    // valid schema state will rebuild the extension-backed helper tables.
  }
}

async function refreshMaterializedRollups(db: Database): Promise<void> {
  await db.run("DELETE FROM _tts_rollups WHERE rollup_table = 'samples_1m'");
  await db.run(
    `INSERT INTO _tts_rollups
     (source_table, rollup_table, metric_name, tags_json, bucket_ns, sample_count,
      min_value_real, max_value_real, sum_value_real, avg_value_real)
     SELECT '_tts_samples', 'samples_1m', s.metric_name, COALESCE(s.tags_json, '{}'),
       c.chunk_start_ns,
       seg.row_count,
       seg.min_value_real,
       seg.max_value_real,
       seg.sum_value_real,
       seg.sum_value_real / seg.row_count
     FROM _tts_segments seg
     JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id
     JOIN _tts_series s ON s.series_id = seg.series_id
     ORDER BY c.chunk_start_ns`,
  );
}

async function refreshColumnarSegments(db: Database, rawSamples: unknown[]): Promise<void> {
  await db.run(
    `INSERT INTO _tts_hypertables (table_name, time_column, chunk_interval_ns, storage_layout)
     VALUES ('_tts_samples', 'ts_ns', ?, 'columnar')
     ON CONFLICT(table_name) DO UPDATE SET
       chunk_interval_ns = excluded.chunk_interval_ns,
       storage_layout = 'columnar'`,
    defaultBucketWidthNs,
  );

  await db.exec(`
    DELETE FROM _tts_segment_columns
      WHERE segment_id IN (
        SELECT seg.segment_id
        FROM _tts_segments seg
        JOIN _tts_chunks c ON c.chunk_id = seg.chunk_id
        JOIN _tts_hypertables h ON h.hypertable_id = c.hypertable_id
        WHERE h.table_name = '_tts_samples'
      );
    DELETE FROM _tts_segments
      WHERE chunk_id IN (
        SELECT c.chunk_id
        FROM _tts_chunks c
        JOIN _tts_hypertables h ON h.hypertable_id = c.hypertable_id
        WHERE h.table_name = '_tts_samples'
      );
    DELETE FROM _tts_chunks
      WHERE hypertable_id IN (
        SELECT hypertable_id FROM _tts_hypertables WHERE table_name = '_tts_samples'
      );
  `);

  const hypertable = (await db.get(
    "SELECT hypertable_id FROM _tts_hypertables WHERE table_name = '_tts_samples'",
  )) as { hypertable_id: number | bigint } | undefined;
  if (!hypertable) {
    return;
  }

  const groups = new Map<string, JsonRow[]>();
  for (const sample of normalizeRows(rawSamples)) {
    const tsNs = BigInt(String(sample.ts_ns));
    const bucketNs = tsNs - (tsNs % defaultBucketWidthNs);
    const key = `${sample.series_id}:${bucketNs.toString()}`;
    const group = groups.get(key) ?? [];
    group.push({ ...sample, bucket_ns: bucketNs.toString() });
    groups.set(key, group);
  }

  for (const group of groups.values()) {
    group.sort((a, b) => (BigInt(String(a.ts_ns)) < BigInt(String(b.ts_ns)) ? -1 : 1));

    const first = group[0];
    const seriesId = first.series_id;
    const chunkStartNs = BigInt(String(first.bucket_ns));
    const chunkEndNs = chunkStartNs + defaultBucketWidthNs;
    const tsValues = group.map((sample) => BigInt(String(sample.ts_ns)));
    const realValues = group.map((sample) => Number(sample.value_real));
    const qualityValues = group.map((sample) => Number(sample.quality));
    const minValue = Math.min(...realValues);
    const maxValue = Math.max(...realValues);
    const sumValue = realValues.reduce((sum, value) => sum + value, 0);

    await db.run(
      `INSERT OR IGNORE INTO _tts_chunks
       (hypertable_id, series_id, chunk_start_ns, chunk_end_ns)
       VALUES (?, ?, ?, ?)`,
      hypertable.hypertable_id,
      seriesId,
      chunkStartNs,
      chunkEndNs,
    );
    const chunk = (await db.get(
      `SELECT chunk_id FROM _tts_chunks
       WHERE hypertable_id = ? AND series_id = ? AND chunk_start_ns = ?`,
      hypertable.hypertable_id,
      seriesId,
      chunkStartNs,
    )) as { chunk_id: number | bigint } | undefined;
    if (!chunk) {
      continue;
    }

    await db.run(
      `INSERT INTO _tts_segments
       (chunk_id, series_id, segment_start_ns, segment_end_ns, row_count,
        min_value_real, max_value_real, sum_value_real)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(chunk_id, series_id, segment_start_ns, segment_end_ns)
       DO UPDATE SET
         row_count = excluded.row_count,
         min_value_real = excluded.min_value_real,
         max_value_real = excluded.max_value_real,
         sum_value_real = excluded.sum_value_real`,
      chunk.chunk_id,
      seriesId,
      tsValues[0],
      tsValues[tsValues.length - 1],
      group.length,
      minValue,
      maxValue,
      sumValue,
    );
    const segment = (await db.get(
      `SELECT segment_id FROM _tts_segments
       WHERE chunk_id = ? AND series_id = ? AND segment_start_ns = ? AND segment_end_ns = ?`,
      chunk.chunk_id,
      seriesId,
      tsValues[0],
      tsValues[tsValues.length - 1],
    )) as { segment_id: number | bigint } | undefined;
    if (!segment) {
      continue;
    }

    for (const [columnName, valueType, encoding, dataBlob] of [
      ['ts_ns', 'integer', 'i64-le', encodeI64LeColumn(tsValues)],
      ['value_real', 'real', 'f64-le', encodeF64LeColumn(realValues)],
      ['quality', 'integer', 'i32-le', encodeI32LeColumn(qualityValues)],
    ] as const) {
      await db.run(
        `INSERT OR REPLACE INTO _tts_segment_columns
         (segment_id, column_name, value_type, encoding, data_blob, null_count)
         VALUES (?, ?, ?, ?, ?, 0)`,
        segment.segment_id,
        columnName,
        valueType,
        encoding,
        dataBlob,
      );
    }
  }
}

function encodeI64LeColumn(values: bigint[]): Uint8Array {
  const out = new ArrayBuffer(values.length * 8);
  const view = new DataView(out);
  values.forEach((value, index) => view.setBigInt64(index * 8, value, true));
  return new Uint8Array(out);
}

function encodeI32LeColumn(values: number[]): Uint8Array {
  const out = new ArrayBuffer(values.length * 4);
  const view = new DataView(out);
  values.forEach((value, index) => view.setInt32(index * 4, value, true));
  return new Uint8Array(out);
}

function encodeF64LeColumn(values: number[]): Uint8Array {
  const out = new ArrayBuffer(values.length * 8);
  const view = new DataView(out);
  values.forEach((value, index) => view.setFloat64(index * 8, value, true));
  return new Uint8Array(out);
}

async function extensionForDatabase(db: Database): Promise<TimeseriesExtension> {
  const existing = extensionsByDatabase.get(db);
  if (existing) {
    return existing;
  }

  const extension = await loadTimeseriesExtension(db);
  extensionsByDatabase.set(db, extension);
  return extension;
}

function renderResult(
  resultNode: HTMLElement,
  meta: HTMLElement,
  result: Exclude<SandboxQueryResult, { ok: false }>,
): void {
  resultNode.textContent = '';

  if (result.kind === 'rows') {
    meta.textContent = `${result.rows.length} row${result.rows.length === 1 ? '' : 's'} in ${formatMs(
      result.elapsedMs,
    )}`;
    renderRows(resultNode, result.columns, result.rows);
    return;
  }

  if (result.kind === 'run') {
    meta.textContent = `changed ${result.changes} row${result.changes === 1 ? '' : 's'} in ${formatMs(
      result.elapsedMs,
    )}`;
    resultNode.textContent = `lastInsertRowid: ${result.lastInsertRowid}`;
    return;
  }

  meta.textContent = `executed in ${formatMs(result.elapsedMs)}`;
  resultNode.textContent = 'ok';
}

function renderRows(resultNode: HTMLElement, columns: string[], rows: JsonRow[]): void {
  if (columns.length === 0) {
    resultNode.textContent = 'No columns.';
    return;
  }

  const table = document.createElement('table');
  const thead = document.createElement('thead');
  const headerRow = document.createElement('tr');
  for (const column of columns) {
    const th = document.createElement('th');
    th.textContent = column;
    headerRow.append(th);
  }
  thead.append(headerRow);
  table.append(thead);

  const tbody = document.createElement('tbody');
  for (const row of rows) {
    const tr = document.createElement('tr');
    for (const column of columns) {
      const td = document.createElement('td');
      td.textContent = formatCell(row[column]);
      tr.append(td);
    }
    tbody.append(tr);
  }
  table.append(tbody);
  resultNode.append(table);
}

function renderError(resultNode: HTMLElement, meta: HTMLElement, message: string, elapsedMs: number): void {
  meta.textContent = elapsedMs > 0 ? `error in ${formatMs(elapsedMs)}` : 'error';
  resultNode.textContent = message;
}

function normalizeRows(rows: unknown[]): JsonRow[] {
  return rows.map((row) => normalizeValue(row) as JsonRow);
}

function normalizeValue(value: unknown): unknown {
  if (typeof value === 'bigint') {
    return value.toString();
  }

  if (Array.isArray(value)) {
    return value.map(normalizeValue);
  }

  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, nestedValue]) => [key, normalizeValue(nestedValue)]),
    );
  }

  return value;
}

function columnNames(statement: { columns(): { name: string }[] }, rows: JsonRow[]): string[] {
  const columns = statement.columns().map((column) => column.name);
  if (columns.length > 0) {
    return columns;
  }

  return rows[0] ? Object.keys(rows[0]) : [];
}

function formatCell(value: unknown): string {
  if (value === null) {
    return 'NULL';
  }

  if (value === undefined) {
    return '';
  }

  if (value instanceof Uint8Array) {
    return `0x${Array.from(value, (byte) => byte.toString(16).padStart(2, '0')).join('')}`;
  }

  if (typeof value === 'object') {
    return JSON.stringify(value);
  }

  return String(value);
}

function elapsedSince(start: number): number {
  return performance.now() - start;
}

function formatMs(ms: number): string {
  return `${Math.max(0, ms).toFixed(1)} ms`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function mustFind<T extends Element>(root: ParentNode, selector: string): T {
  const element = root.querySelector<T>(selector);
  if (!element) {
    throw new Error(`missing sandbox element: ${selector}`);
  }
  return element;
}

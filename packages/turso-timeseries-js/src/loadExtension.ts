export type WasmDatabase = {
  exec(sql: string): Promise<void>;
};

export type LoadTimeseriesExtensionOptions = {
  wasmUrl: string;
};

export async function loadTimeseriesExtension(
  db: WasmDatabase,
  options: LoadTimeseriesExtensionOptions,
): Promise<void> {
  const bytes = new Uint8Array(await fetch(options.wasmUrl).then((r) => r.arrayBuffer()));
  const hex = [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');
  await db.exec(`
    CREATE EXTENSION IF NOT EXISTS turso_timeseries
    LANGUAGE wasm
    AS X'${hex}'
  `);
}

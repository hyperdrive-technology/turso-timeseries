import type { WasmDatabase } from './loadExtension.js';

/** Apply embedded catalog DDL shipped with the host app (see `turso-timeseries-catalog`). */
export async function installTimeseriesCatalog(
  db: WasmDatabase,
  migrations: string[],
): Promise<void> {
  for (const sql of migrations) {
    await db.exec(sql);
  }
}

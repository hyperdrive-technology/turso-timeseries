import type { WasmDatabase } from './loadExtension.js';

export async function writeLineProtocol(db: WasmDatabase, line: string): Promise<void> {
  await db.exec(`SELECT 1`);
  void line;
  throw new Error(
    'writeLineProtocol requires host-side ingest until tts_write_line_protocol WASM scalar ships',
  );
}

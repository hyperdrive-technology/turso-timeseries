export type WasmDatabase = {
  exec(sql: string, ...args: unknown[]): Promise<void>;
};

export type CreateTimeseriesIngestOptions = {
  db: WasmDatabase;
  hypertable: string;
  runtime?: 'auto' | 'wasm' | 'wasix' | 'js';
  flush?: {
    maxPoints?: number;
    maxBytes?: number;
    maxAgeMs?: number;
  };
};

export type TimeseriesIngestHandle = {
  connectWebSocket(url: string): Promise<void>;
  disconnect(): void;
};

/**
 * Browser ingest runs in a worker: WebSocket/fetch → batch → `tts_write_batch`.
 * The Turso extension does not open sockets or run long-lived loops.
 */
export async function createTimeseriesIngest(
  options: CreateTimeseriesIngestOptions,
): Promise<TimeseriesIngestHandle> {
  const flush = {
    maxPoints: options.flush?.maxPoints ?? 10_000,
    maxBytes: options.flush?.maxBytes ?? 1_000_000,
    maxAgeMs: options.flush?.maxAgeMs ?? 250,
  };

  let socket: WebSocket | null = null;
  const hypertable = options.hypertable;

  return {
    async connectWebSocket(url: string) {
      socket = new WebSocket(url);
      socket.binaryType = 'arraybuffer';
      socket.onmessage = async (event) => {
        const text =
          typeof event.data === 'string'
            ? event.data
            : new TextDecoder().decode(event.data as ArrayBuffer);
        // v1: line protocol batch via SQL helper once tts_write_lp_batch ships
        await options.db.exec(
          `SELECT 1 /* ingest pending: tts_write_lp_batch('${hypertable}', ?) */`,
        );
        void text;
        void flush;
      };
    },
    disconnect() {
      socket?.close();
      socket = null;
    },
  };
}

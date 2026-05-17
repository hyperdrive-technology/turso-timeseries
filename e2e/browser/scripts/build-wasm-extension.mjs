import { mkdir, writeFile } from 'node:fs/promises';
import wabtFactory from 'wabt';

const wabt = await wabtFactory();
const manifest = JSON.stringify({
  functions: [
    { name: 'tts_extension_loaded', export: 'tts_extension_loaded', narg: 0 },
    { name: 'tts_time_bucket_ns', export: 'tts_time_bucket_ns', narg: 2 },
  ],
});
const manifestBytes = new TextEncoder().encode(manifest);
const manifestWat = [...manifestBytes].map((byte) => `\\${byte.toString(16).padStart(2, '0')}`).join('');

const source = `
(module
  (memory (export "memory") 1)
  (global $bump (mut i32) (i32.const 4096))
  (data (i32.const 1024) "${manifestWat}")

  (func (export "turso_malloc") (param $size i32) (result i32)
    (local $ptr i32)
    global.get $bump
    local.set $ptr
    global.get $bump
    local.get $size
    i32.add
    global.set $bump
    local.get $ptr)

  (func (export "turso_ext_init") (param $argc i32) (param $argv i32) (result i64)
    (local $ptr i32)
    (local $i i32)
    global.get $bump
    local.set $ptr
    global.get $bump
    i32.const ${manifestBytes.length + 2}
    i32.add
    global.set $bump

    local.get $ptr
    i32.const 3
    i32.store8

    i32.const 0
    local.set $i
    (block $done
      (loop $copy
        local.get $i
        i32.const ${manifestBytes.length}
        i32.ge_u
        br_if $done
        local.get $ptr
        i32.const 1
        i32.add
        local.get $i
        i32.add
        i32.const 1024
        local.get $i
        i32.add
        i32.load8_u
        i32.store8
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $copy))

    local.get $ptr
    i32.const 1
    i32.add
    i32.const ${manifestBytes.length}
    i32.add
    i32.const 0
    i32.store8

    local.get $ptr
    i64.extend_i32_u)

  (func (export "tts_extension_loaded") (param $argc i32) (param $argv i32) (result i64)
    i64.const 1)

  (func (export "tts_time_bucket_ns") (param $argc i32) (param $argv i32) (result i64)
    (local $ts i64)
    (local $width i64)
    local.get $argv
    i64.load
    local.set $ts
    local.get $argv
    i32.const 8
    i32.add
    i64.load
    local.set $width
    local.get $width
    i64.const 0
    i64.le_s
    if (result i64)
      i64.const 0
    else
      local.get $ts
      local.get $ts
      local.get $width
      i64.rem_s
      i64.sub
    end)

  (func (export "tts_time_bucket_ns_direct") (param $ts i64) (param $width i64) (result i64)
    local.get $ts
    local.get $ts
    local.get $width
    i64.rem_s
    i64.sub))
`;

const module = wabt.parseWat('tts_extension.wat', source);
module.validate();
const { buffer } = module.toBinary({ log: false, write_debug_names: true });

await mkdir(new URL('../public/', import.meta.url), { recursive: true });
await writeFile(new URL('../public/tts_extension.wasm', import.meta.url), Buffer.from(buffer));

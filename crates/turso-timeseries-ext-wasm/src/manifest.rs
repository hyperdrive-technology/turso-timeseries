/// JSON manifest returned from `turso_ext_init`.
pub const MANIFEST_JSON: &str = r#"{
  "functions": [
    { "name": "tts_version", "export": "tts_version", "narg": 0 },
    { "name": "time_bucket", "export": "time_bucket", "narg": 2 },
    { "name": "tts_parse_duration_micros", "export": "tts_parse_duration_micros", "narg": 1 }
  ]
}"#;

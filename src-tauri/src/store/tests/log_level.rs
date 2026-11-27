//! The log level in `state.json` (#109): the old „Debug“ switch taken over, unknown levels
//! read as „Info“.
use super::*;
use crate::app_log::Level;

#[test]
fn the_log_level_takes_over_a_stored_debug_switch_and_reads_an_unknown_one_as_info() {
    let dir = TempDir::new("log-level");
    let level = |settings: Value| {
        dir.write(&json!({"version": 1, "settings": settings}).to_string());
        load(&dir.0).state.settings.log_level
    };
    // A file from before #109: „Debug“ (#41) on stays „Debug“, off or absent is „Info“.
    assert_eq!(level(json!({"debug": true})), Level::Debug);
    assert_eq!(level(json!({"debug": false})), Level::Info);
    assert_eq!(level(json!({})), Level::Info);
    // `logLevel` wins over the old switch once it is written.
    assert_eq!(
        level(json!({"logLevel": "info", "debug": true})),
        Level::Info
    );
    assert_eq!(level(json!({"logLevel": "debug"})), Level::Debug);
    // A newer build's level (or junk) reads as „Info“, the other settings stay.
    for unknown in [json!("trace"), json!(1), Value::Null] {
        dir.write(
            &json!({"version": 1, "settings": {"logLevel": unknown, "sharedWindow": true}})
                .to_string(),
        );
        let loaded = load(&dir.0).state.settings;
        assert_eq!(loaded.log_level, Level::Info);
        assert!(loaded.shared_window);
    }
}

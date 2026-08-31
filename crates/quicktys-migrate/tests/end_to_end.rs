//! End-to-end: a small CastMate-shaped data dir migrates to on-disk Quicktys
//! resources that **round-trip back into the real Quicktys types**, plus a
//! report. Proves the migrator's output is valid Quicktys, not just plausible.

use quicktys_core::{Automation, Profile};
use quicktys_migrate::{migrate, Kind};
use std::fs;

/// A throwaway temp dir under the system temp, unique per test name.
fn tmp(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("qtmigrate-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn migrates_profile_and_automation_to_valid_quicktys() {
    let root = tmp("e2e");
    let src = root.join("castmate");
    let out = root.join("out");
    fs::create_dir_all(src.join("profiles")).unwrap();
    fs::create_dir_all(src.join("automations")).unwrap();

    // A CastMate-shaped profile: a trigger whose sequence uses both action
    // spellings and a lone-path template (→ typed `@`).
    fs::write(
        src.join("profiles/main.yaml"),
        r#"
name: Main
triggers:
  - trigger: twitch.chat
    sequence:
      - obs.scene:
          scene: "{{ pear.title }}"
      - plugin: notify
        action: toast
        text: hello
"#,
    )
    .unwrap();

    // A standalone automation (bare sequence).
    fs::write(
        src.join("automations/intro.yaml"),
        r#"
sequence:
  - notify.toast:
      text: intro
"#,
    )
    .unwrap();

    let report = migrate(&src, &out).expect("migrate");

    assert_eq!(report.migrated(Kind::Profile), 1);
    assert_eq!(report.migrated(Kind::Automation), 1);
    assert_eq!(report.action_count(), 3);

    // The written profile parses back into the real Quicktys Profile type.
    let ptxt = fs::read_to_string(out.join("profiles/main.yaml")).unwrap();
    let profile: Profile = serde_yaml::from_str(&ptxt).expect("valid Quicktys Profile");
    assert_eq!(profile.id, "main");
    assert_eq!(profile.name.as_deref(), Some("Main"));
    assert_eq!(profile.triggers.len(), 1);
    assert_eq!(profile.triggers[0].trigger, "twitch.chat");
    // The lone `{{ pear.title }}` became the typed `@pear.title`.
    assert!(
        ptxt.contains("@pear.title"),
        "expected typed @-var in output, got:\n{ptxt}"
    );

    // The automation parses back into the real Quicktys Automation type.
    let atxt = fs::read_to_string(out.join("automations/intro.yaml")).unwrap();
    let auto: Automation = serde_yaml::from_str(&atxt).expect("valid Quicktys Automation");
    assert_eq!(auto.id, "intro");

    // A migration report was written.
    assert!(out.join("migration-report.md").is_file());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unknown_shape_is_reported_not_written() {
    let root = tmp("unknown");
    let src = root.join("castmate");
    let out = root.join("out");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("settings.yaml"), "theme: dark\nvolume: 5\n").unwrap();

    let report = migrate(&src, &out).expect("migrate");
    assert_eq!(report.migrated(Kind::Profile), 0);
    assert_eq!(report.migrated(Kind::Automation), 0);
    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.entries[0].kind, Kind::Unknown);
    assert!(report.entries[0].written.is_none());
    assert!(report.needs_review());

    let _ = fs::remove_dir_all(&root);
}

//! The shared directories: linked, adopted with a backup, never overwritten.
use super::*;
use crate::profiles::links::{inspect, Found};

fn shared() -> &'static [&'static str] {
    recipe(CLAUDE).unwrap().shared_dirs
}

#[test]
fn the_first_sync_links_every_shared_directory_and_writes_the_marker() {
    let world = World::new("profiles-links");
    write(
        &world.dir("default").join("skills/review/SKILL.md"),
        "review",
    );
    let reports = world.synced(&["team"], 1);
    assert!(reports[0].changed && !reports[0].dir_failed);
    for name in shared() {
        let (link, target) = (
            world.dir("team").join(name),
            world.dir("default").join(name),
        );
        assert!(target.is_dir(), "{name} is created in the default home");
        assert_eq!(inspect(&link, &target).unwrap(), Found::Linked, "{name}");
    }
    // One directory, two names: what a session of one profile writes, the other one sees.
    assert_eq!(
        read(&world.dir("team").join("skills/review/SKILL.md")),
        "review"
    );
    write(&world.dir("team").join("plans/next.md"), "plan");
    assert_eq!(read(&world.dir("default").join("plans/next.md")), "plan");
    let marker = read_marker(&world.dir("team")).unwrap();
    assert_eq!(
        (marker.version, marker.synced_at.as_deref()),
        (1, Some("2026-11-13T09:01:00Z"))
    );
    assert!(!world
        .dir("team")
        .join(super::super::files::BACKUP_DIR)
        .exists());
}

#[test]
fn a_profile_directory_is_adopted_into_the_default_home_and_backed_up() {
    let world = World::new("profiles-adopt");
    let (ours, theirs) = (world.dir("default"), world.dir("team"));
    write(&ours.join("skills/shared.md"), "default version");
    write(&ours.join("skills/nested/kept.md"), "kept");
    write(&theirs.join("skills/shared.md"), "team version");
    write(&theirs.join("skills/only-team.md"), "team only");
    write(&theirs.join("skills/nested/new.md"), "nested new");
    // A file where the default home has a directory, and the other way round: left alone.
    write(&ours.join("skills/clash"), "a file");
    write(&theirs.join("skills/clash/inner.md"), "inner");
    touch(&theirs.join("skills/only-team.md"), 42);
    std::fs::create_dir_all(theirs.join("agents")).unwrap();
    world.synced(&["team"], 2);
    // Missing entries arrived, with their file time; nothing existing was overwritten.
    assert_eq!(read(&ours.join("skills/only-team.md")), "team only");
    assert_eq!(read(&ours.join("skills/nested/new.md")), "nested new");
    assert_eq!(read(&ours.join("skills/shared.md")), "default version");
    assert_eq!(read(&ours.join("skills/nested/kept.md")), "kept");
    assert_eq!(read(&ours.join("skills/clash")), "a file");
    let time = |path: &Path| std::fs::metadata(path).unwrap().modified().unwrap();
    assert_eq!(
        time(&ours.join("skills/only-team.md")),
        time(&world.backup("team").join("skills/only-team.md"))
    );
    // The profile's own directory moved to the backup whole, its version of the clash too.
    let backup = world.backup("team");
    assert_eq!(
        backup.file_name().unwrap().to_str().unwrap(),
        "20261113T090200Z"
    );
    assert_eq!(read(&backup.join("skills/shared.md")), "team version");
    assert_eq!(read(&backup.join("skills/clash/inner.md")), "inner");
    assert_eq!(read(&backup.join("skills/nested/new.md")), "nested new");
    // The empty directory was simply replaced: no backup of it.
    assert!(!backup.join("agents").exists());
    for name in ["skills", "agents"] {
        assert_eq!(
            inspect(&theirs.join(name), &ours.join(name)).unwrap(),
            Found::Linked
        );
    }
    // No temp file stays behind.
    let leftovers: Vec<_> = tree(world.home())
        .into_iter()
        .filter(|(path, _, _)| path.to_string_lossy().contains(".traygent-tmp-"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_link_that_points_elsewhere_is_left_alone_and_reported() {
    let world = World::new("profiles-elsewhere");
    let elsewhere = world.home().join("dotfiles-skills");
    write(&elsewhere.join("mine.md"), "mine");
    std::fs::create_dir_all(world.dir("team")).unwrap();
    let foreign = Link {
        link: world.dir("team").join("skills"),
        target: elsewhere.clone(),
    };
    linker().link(std::slice::from_ref(&foreign));
    assert_eq!(inspect(&foreign.link, &elsewhere).unwrap(), Found::Linked);
    // A file where a directory belongs is not replaced either.
    write(&world.dir("team").join("commands"), "not a directory");
    let reports = world.sync(&["team"], 3);
    assert_eq!(
        reports[0].errors,
        [
            "skills: Verknüpfung zeigt auf ein anderes Ziel",
            "commands: ist kein Ordner"
        ]
    );
    assert!(!reports[0].dir_failed);
    assert_eq!(inspect(&foreign.link, &elsewhere).unwrap(), Found::Linked);
    assert_eq!(read(&elsewhere.join("mine.md")), "mine");
    assert_eq!(read(&world.dir("team").join("commands")), "not a directory");
    assert!(!world.dir("default").join("skills/mine.md").exists());
    // The other directories are linked all the same.
    let (link, target) = (
        world.dir("team").join("plans"),
        world.dir("default").join("plans"),
    );
    assert_eq!(inspect(&link, &target).unwrap(), Found::Linked);
    // The first sync happened (the default home wins only once), but not without an error.
    let marker = read_marker(&world.dir("team")).unwrap();
    assert_eq!((marker.version, marker.synced_at), (1, None));
}

#[test]
fn a_profile_directory_that_is_the_default_home_is_refused() {
    let world = World::new("profiles-alias");
    write(&world.dir("default").join("skills/own.md"), "own");
    let alias = Link {
        link: world.dir("team"),
        target: world.dir("default"),
    };
    linker().link(std::slice::from_ref(&alias));
    let before = tree(&world.dir("default"));
    let reports = world.sync(&["team"], 4);
    assert_eq!(
        reports[0].errors,
        ["Profilordner ist der Ordner des Standard-Profils"]
    );
    // Nothing was adopted, moved or marked: the default home is as it was.
    assert_eq!(tree(&world.dir("default")), before);
    assert!(world.dir("default").join("skills").is_dir());
    assert!(!world.dir("default").join(MARKER).exists());
}

#[test]
fn a_profile_directory_that_cannot_be_created_is_reported_as_such() {
    let world = World::new("profiles-blocked");
    write(&world.dir("team"), "a file in the way");
    let reports = world.sync(&["team", "solo"], 5);
    assert!(reports[0].dir_failed);
    assert_eq!(
        reports[0].errors,
        ["Profilordner konnte nicht angelegt werden"]
    );
    assert_eq!(read(&world.dir("team")), "a file in the way");
    // The other profile is synced.
    assert!(reports[1].errors.is_empty() && !reports[1].dir_failed);
    assert!(read_marker(&world.dir("solo")).is_some());
}

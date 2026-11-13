//! The user's extra shared directories and files (#35): they behave like the recipe's own,
//! and no name reaches a path unchecked.
use super::*;
use crate::profiles::share::{check_name, NameError, Sharing, SKIPPED};
use crate::profiles::sync::refuses_sign_in;

fn share(dirs: &[&str], files: &[&str]) -> ProfileShare {
    let owned = |list: &[&str]| list.iter().map(|name| name.to_string()).collect();
    ProfileShare {
        dirs: owned(dirs),
        files: owned(files),
    }
}

fn is_link(path: &Path) -> bool {
    let meta = path.symlink_metadata();
    meta.is_ok_and(|meta| meta.file_type().is_symlink())
}

#[test]
fn an_extra_directory_is_linked_and_an_extra_file_synced_like_the_recipes_own() {
    let world = World::new("share-extras");
    let extras = share(&[".caveman-sessions"], &["orca-state.json"]);
    let (ours, theirs) = (world.dir("default"), world.dir("team"));
    // The profile already has the directory: adopted into the default home, backed up,
    // then one directory under two names.
    write(&theirs.join(".caveman-sessions/team.txt"), "team");
    write(&ours.join(".caveman-sessions/default.txt"), "default");
    write(&ours.join("orca-state.json"), "default state");
    write(&theirs.join("orca-state.json"), "team state");
    touch(&ours.join("orca-state.json"), 10);
    touch(&theirs.join("orca-state.json"), 500);
    own(&world, "team");
    let reports = world.sync_sharing(&["team", "solo"], &extras, 1);
    for report in &reports {
        assert_eq!(report.errors, Vec::<String>::new(), "{}", report.id);
    }
    for id in ["team", "solo"] {
        assert!(is_link(&world.dir(id).join(".caveman-sessions")), "{id}");
        assert_eq!(
            read(&world.dir(id).join(".caveman-sessions/default.txt")),
            "default"
        );
        // The first sync: the default home's file wins, even against a newer one.
        assert_eq!(
            read(&world.dir(id).join("orca-state.json")),
            "default state"
        );
    }
    assert_eq!(read(&ours.join(".caveman-sessions/team.txt")), "team");
    let backup = world.backup("team");
    assert_eq!(read(&backup.join(".caveman-sessions/team.txt")), "team");
    assert_eq!(read(&backup.join("orca-state.json")), "team state");
    let marker = read_marker(&theirs).unwrap();
    assert!(marker.done.iter().any(|rule| rule == "orca-state.json"));
    // Later the newer file wins, in either direction, and reaches the third profile.
    write(&theirs.join("orca-state.json"), "team edit");
    touch(&ours.join("orca-state.json"), 600);
    touch(&world.dir("solo").join("orca-state.json"), 600);
    touch(&theirs.join("orca-state.json"), 700);
    world.sync_sharing(&["team", "solo"], &extras, 2);
    for id in ["default", "team", "solo"] {
        assert_eq!(read(&world.dir(id).join("orca-state.json")), "team edit");
    }
    // Without the setting none of this happens.
    let plain = World::new("share-none");
    write(&plain.dir("default").join("orca-state.json"), "x");
    write(&plain.dir("default").join(".caveman-sessions/a.txt"), "x");
    plain.synced(&["team"], 1);
    assert!(!plain.dir("team").join("orca-state.json").exists());
    assert!(!plain.dir("team").join(".caveman-sessions").exists());
}

#[test]
fn a_removed_extra_leaves_its_link_and_its_file_where_they_are() {
    let world = World::new("share-removed");
    let extras = share(&["notes"], &["orca-state.json"]);
    write(&world.dir("default").join("notes/a.md"), "a");
    write(&world.dir("default").join("orca-state.json"), "v1");
    world.sync_sharing(&["team"], &extras, 1);
    let (link, file) = (
        world.dir("team").join("notes"),
        world.dir("team").join("orca-state.json"),
    );
    assert!(is_link(&link));
    // The name is taken out of the setting: nothing is undone, the file just stops syncing.
    write(&world.dir("default").join("orca-state.json"), "v2");
    let reports = world.synced(&["team"], 2);
    assert!(!reports[0].changed);
    assert!(is_link(&link));
    assert_eq!(read(&link.join("a.md")), "a");
    assert_eq!(read(&file), "v1");
    assert_eq!(read(&world.dir("default").join("notes/a.md")), "a");
}

#[test]
fn the_sync_checks_every_extra_name_again_and_skips_what_fails() {
    let recipe = recipe(CLAUDE).unwrap();
    assert_eq!(check_name(recipe, "notes"), Ok(()));
    assert_eq!(check_name(recipe, ".."), Err(NameError::Invalid));
    assert_eq!(check_name(recipe, "a/b"), Err(NameError::Invalid));
    assert_eq!(
        check_name(recipe, ".traygent-base"),
        Err(NameError::Invalid)
    );
    assert_eq!(
        check_name(recipe, ".Credentials.json"),
        Err(NameError::Handled)
    );
    assert_eq!(check_name(recipe, "settings.json"), Err(NameError::Handled));
    // What a plan built from unchecked settings would hold.
    let many: Vec<String> = (0..17).map(|index| format!("n{index}")).collect();
    let mut bad = share(
        &[
            "notes",
            "..",
            "..\\outside",
            "/etc",
            "skills",
            "NOTES",
            "bad\u{7}name",
        ],
        &["notes", ".credentials.json", "sub/file", "orca-state.json"],
    );
    let sharing = Sharing::of(recipe, &bad);
    assert_eq!(
        sharing.dirs[..recipe.shared_dirs.len()],
        *recipe.shared_dirs
    );
    assert_eq!(sharing.dirs[recipe.shared_dirs.len()..], ["notes"]);
    assert_eq!(
        sharing.files[recipe.synced_files.len()..],
        ["orca-state.json"]
    );
    assert_eq!(sharing.skipped.len(), 9);
    assert!(sharing.skipped.contains(&format!("badname: {SKIPPED}")));
    let capped = ProfileShare {
        dirs: many.clone(),
        files: Vec::new(),
    };
    assert_eq!(
        Sharing::of(recipe, &capped).skipped,
        [format!("n16: {SKIPPED}")]
    );

    // Through the sync: each skipped name is a `syncError`, and nothing outside the two
    // homes is created, read or linked.
    let world = World::new("share-recheck");
    let outside = world.home().join("outside");
    write(&outside.join("secret.txt"), "outside");
    write(&world.dir("default").join("notes/a.md"), "a");
    write(&world.dir("default").join("orca-state.json"), "v1");
    write(&world.dir("default").join("sub/file"), "nested");
    write(
        &world.dir("default").join(".credentials.json"),
        "synthetic-default",
    );
    bad.dirs.push("..".into());
    let before = tree(&outside);
    let reports = world.sync_sharing(&["team"], &bad, 1);
    assert_eq!(reports[0].errors.len(), 10, "{:?}", reports[0].errors);
    assert!(reports[0]
        .errors
        .iter()
        .all(|error| error.ends_with(SKIPPED)));
    assert_eq!(tree(&outside), before);
    let team = world.dir("team");
    assert!(is_link(&team.join("notes")));
    assert_eq!(read(&team.join("orca-state.json")), "v1");
    assert!(!team.join(".credentials.json").exists());
    assert!(!team.join("sub").exists());
    let mut entries: Vec<_> = std::fs::read_dir(world.home())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    entries.sort();
    assert_eq!(entries, [".claude", ".claude-team", "outside"]);
}

/// A sync stopped before its marker leaves nothing but links: with an extra directory among
/// them the profile directory still is the agent's own.
#[test]
fn a_directory_of_links_to_extra_directories_is_not_foreign() {
    let world = World::new("share-foreign");
    let default = world.dir("default");
    let extras = share(&["notes"], &[]);
    for name in ["notes", "plans"] {
        let pair = Link {
            link: world.dir("team").join(name),
            target: default.join(name),
        };
        std::fs::create_dir_all(&pair.target).unwrap();
        std::fs::create_dir_all(world.dir("team")).unwrap();
        linker().link(std::slice::from_ref(&pair));
    }
    assert!(!refuses_sign_in(
        CLAUDE,
        &world.dir("team"),
        &default,
        &extras
    ));
    // Without the setting `notes` is something unknown.
    let none = ProfileShare::default();
    assert!(refuses_sign_in(CLAUDE, &world.dir("team"), &default, &none));
    assert!(world.sync(&["team"], 1)[0].foreign);
    let reports = world.sync_sharing(&["team"], &extras, 2);
    assert_eq!(reports[0].errors, Vec::<String>::new());
    assert!(world.dir("team").join(MARKER).exists());
}

#[test]
fn a_sign_in_is_refused_only_for_a_directory_that_is_someone_elses() {
    let world = World::new("profiles-sign-in");
    let default = world.dir("default");
    let refuses =
        |id: &str| refuses_sign_in(CLAUDE, &world.dir(id), &default, &ProfileShare::default());
    assert!(!refuses("missing"));
    std::fs::create_dir_all(world.dir("empty")).unwrap();
    assert!(!refuses("empty"));
    write(&world.state("own"), "{}");
    write(&world.dir("own").join("notes.md"), "x");
    assert!(!refuses("own"));
    write(&world.dir("mem").join("notes.md"), "someone else's");
    assert!(refuses("mem"));
    write(&world.dir("file"), "a file in the way");
    assert!(refuses("file"));
    assert!(refuses_sign_in(
        AgentId::Codex,
        &world.dir("empty"),
        &default,
        &ProfileShare::default()
    ));
    // The real file system answers the launch preparation's question the same way.
    use crate::detect::{Fs, RealFs};
    let path = |id: &str| world.dir(id).to_str().unwrap().to_string();
    assert!(RealFs.refuses_sign_in(
        CLAUDE,
        &path("mem"),
        &path("default"),
        &ProfileShare::default()
    ));
    assert!(!RealFs.refuses_sign_in(
        CLAUDE,
        &path("own"),
        &path("default"),
        &ProfileShare::default()
    ));
}

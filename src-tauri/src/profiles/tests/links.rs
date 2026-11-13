//! The shared directories: linked, adopted with a backup, never overwritten.
use super::*;
use crate::profiles::links::{inspect, Found};

fn none() -> ProfileShare {
    ProfileShare::default()
}

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
    own(&world, "team");
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
    own(&world, "team");
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
    // No sync without an error yet; the file rules that succeeded had their first sync.
    let marker = read_marker(&world.dir("team")).unwrap();
    assert_eq!((marker.version, marker.synced_at), (1, None));
    assert_eq!(
        marker.done,
        [
            "settings.json",
            "CLAUDE.md",
            "keybindings.json",
            ".claude.json"
        ]
    );
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

/// PowerShell blocked, a timeout: the profile keeps its own directories, and a later sync
/// starts from the same state — no backup piles up.
#[test]
fn a_failed_link_gives_the_profile_its_directory_back() {
    let world = World::new("profiles-nolink");
    let (ours, theirs) = (world.dir("default"), world.dir("team"));
    own(&world, "team");
    write(&theirs.join("skills/team.md"), "team skill");
    std::fs::create_dir_all(theirs.join("agents")).unwrap();
    let profiles = [profile("team")];
    for minute in [1, 2] {
        let reports = sync_agent(
            &world.homes,
            CLAUDE,
            &profiles,
            &none(),
            &crate::testing::NoLinker,
            at(minute),
        );
        assert_eq!(
            reports[0].errors.len(),
            shared().len(),
            "{:?}",
            reports[0].errors
        );
        assert_eq!(
            reports[0].errors[0],
            "skills: Verknüpfung konnte nicht angelegt werden"
        );
        assert!(!reports[0].dir_failed);
        // The non-empty directory is back in place, the removed empty one exists again.
        assert_eq!(
            inspect(&theirs.join("skills"), &ours.join("skills")).unwrap(),
            Found::Directory
        );
        assert_eq!(read(&theirs.join("skills/team.md")), "team skill");
        assert_eq!(
            inspect(&theirs.join("agents"), &ours.join("agents")).unwrap(),
            Found::Directory
        );
        assert_eq!(
            inspect(&theirs.join("plans"), &ours.join("plans")).unwrap(),
            Found::Missing
        );
        assert!(
            !theirs.join(super::super::files::BACKUP_DIR).exists(),
            "run {minute}"
        );
        assert_eq!(read_marker(&theirs).unwrap().synced_at, None);
    }
    // Once linking works the directory is adopted as usual.
    world.synced(&["team"], 3);
    assert_eq!(
        inspect(&theirs.join("skills"), &ours.join("skills")).unwrap(),
        Found::Linked
    );
    assert_eq!(
        read(&world.backup("team").join("skills/team.md")),
        "team skill"
    );
}

#[test]
fn a_foreign_directory_is_left_exactly_as_it_is() {
    let world = World::new("profiles-foreign");
    write(&world.dir("mem").join("skills/notes.md"), "someone else's");
    write(&world.dir("mem").join("keybindings.json"), "theirs");
    write(&world.dir("default").join("keybindings.json"), "ours");
    // An empty directory is nobody's yet, and one with the CLI's own files is the agent's.
    std::fs::create_dir_all(world.dir("fresh")).unwrap();
    write(&world.state("old"), "{}");
    write(&world.dir("old").join("skills/own.md"), "own");
    let before = tree(&world.dir("mem"));
    let reports = world.sync(&["mem", "fresh", "old"], 1);
    assert_eq!(
        reports[0].errors,
        ["Profilordner gehört nicht zu Claude Code"]
    );
    assert!(reports[0].dir_failed && reports[0].foreign);
    assert_eq!(tree(&world.dir("mem")), before);
    assert!(world
        .dir("mem")
        .join("skills")
        .symlink_metadata()
        .unwrap()
        .is_dir());
    assert!(!world.dir("default").join("skills/notes.md").exists());
    for report in &reports[1..] {
        assert_eq!(report.errors, Vec::<String>::new(), "{}", report.id);
        assert!(!report.foreign);
    }
    assert_eq!(read(&world.dir("default").join("skills/own.md")), "own");
}

/// A guard that cannot tell refuses: no answer is never read as "different places".
#[test]
fn an_unanswered_same_place_check_is_an_error_and_a_dangling_link_is_never_touched() {
    use crate::profiles::links::same_place;
    let world = World::new("profiles-same-place");
    let (here, gone) = (world.home().join("here"), world.home().join("gone"));
    std::fs::create_dir_all(&here).unwrap();
    assert!(same_place(&here, &here).unwrap());
    assert!(same_place(&here, &gone).is_err());
    assert!(same_place(&gone, &here).is_err());
    let dangling = Link {
        link: world.home().join("dangling"),
        target: here.clone(),
    };
    linker().link(std::slice::from_ref(&dangling));
    std::fs::remove_dir(&here).unwrap();
    std::fs::create_dir_all(&gone).unwrap();
    assert_eq!(inspect(&dangling.link, &gone).unwrap(), Found::Elsewhere);
}

/// Git pack files in `plugins` are read-only: the copy keeps flag and file time, and the
/// time step does not need to reopen a read-only temp file.
#[test]
fn a_read_only_file_is_adopted_with_its_time_and_flag() {
    use crate::profiles::links::copy_new;
    let world = World::new("profiles-readonly");
    let (source, dest) = (world.home().join("pack.idx"), world.home().join("copy.idx"));
    write(&source, "pack");
    touch(&source, 77);
    let mut read_only = std::fs::metadata(&source).unwrap().permissions();
    read_only.set_readonly(true);
    std::fs::set_permissions(&source, read_only).unwrap();
    copy_new(&source, &dest).unwrap();
    let (from, to) = (
        std::fs::metadata(&source).unwrap(),
        std::fs::metadata(&dest).unwrap(),
    );
    assert_eq!(read(&dest), "pack");
    assert_eq!(to.modified().unwrap(), from.modified().unwrap());
    assert!(to.permissions().readonly());
    let names: Vec<_> = tree(world.home())
        .into_iter()
        .map(|(path, _, _)| path)
        .collect();
    assert_eq!(
        names,
        [dest.clone(), source.clone()],
        "no temp file stays behind"
    );
    // Writable again, so the temp dir can be removed on every platform.
    for path in [&source, &dest] {
        let mut writable = std::fs::metadata(path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        writable.set_readonly(false);
        std::fs::set_permissions(path, writable).unwrap();
    }
}

#[test]
fn the_hidden_linker_never_runs_a_bare_powershell() {
    use crate::profiles::links::JunctionLinker;
    let bare = JunctionLinker {
        powershell: "powershell.exe".into(),
    };
    assert!(!bare.runnable());
    let found = JunctionLinker {
        powershell: crate::testing::windows_host().powershell(),
    };
    assert!(found.runnable());
    // A host without a usable `SystemRoot`: nothing is run, every link reports its failure.
    let mut host = crate::testing::windows_host();
    host.system_root = None;
    let world = World::new("profiles-bare");
    let linker = JunctionLinker {
        powershell: host.powershell(),
    };
    let reports = sync_agent(
        &world.homes,
        CLAUDE,
        &[profile("team")],
        &none(),
        &linker,
        at(1),
    );
    assert_eq!(reports[0].errors.len(), shared().len());
    assert!(reports[0].errors[0].ends_with("Verknüpfung konnte nicht angelegt werden"));
}

/// The one test that creates real junctions through Windows PowerShell.
#[cfg(windows)]
#[test]
fn junctions_are_created_and_recognized_on_windows() {
    let world = World::new("profiles-junction");
    own(&world, "team");
    write(&world.dir("team").join("skills/team.md"), "team skill");
    let linker = system_linker(&crate::detect::Host::current());
    let reports = sync_agent(
        &world.homes,
        CLAUDE,
        &[profile("team")],
        &none(),
        linker.as_ref(),
        at(1),
    );
    assert_eq!(reports[0].errors, Vec::<String>::new());
    for name in shared() {
        let (link, target) = (
            world.dir("team").join(name),
            world.dir("default").join(name),
        );
        assert_eq!(inspect(&link, &target).unwrap(), Found::Linked, "{name}");
    }
    assert_eq!(
        read(&world.dir("default").join("skills/team.md")),
        "team skill"
    );
    let again = sync_agent(
        &world.homes,
        CLAUDE,
        &[profile("team")],
        &none(),
        linker.as_ref(),
        at(2),
    );
    assert!(again[0].errors.is_empty() && !again[0].changed);
}

/// Records what the profile directory holds at the moment the links are made.
struct Watching(std::sync::Mutex<Vec<String>>);

impl Linker for Watching {
    fn link(&self, links: &[Link]) {
        let dir = links[0].link.parent().unwrap();
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        *self.0.lock().unwrap() = names;
        crate::testing::TestLinker.link(links);
    }
}

/// A sync stopped between linking and its marker must not leave a directory that looks
/// like someone else's: a new directory is marked before anything is linked into it.
#[test]
fn a_new_profile_directory_is_marked_before_it_is_linked() {
    let world = World::new("profiles-initial-marker");
    let watching = Watching(Default::default());
    let reports = sync_agent(
        &world.homes,
        CLAUDE,
        &[profile("team")],
        &none(),
        &watching,
        at(1),
    );
    assert_eq!(reports[0].errors, Vec::<String>::new());
    assert_eq!(
        *watching.0.lock().unwrap(),
        [MARKER],
        "the marker is there first"
    );
    // What such a stopped sync leaves — the initial marker and links — is synced on as a
    // first sync: the default home still wins.
    let stopped = World::new("profiles-stopped");
    write(&stopped.dir("default").join("CLAUDE.md"), "ours");
    write(
        &stopped.dir("team").join(MARKER),
        r#"{"version":1,"done":[]}"#,
    );
    let plans = Link {
        link: stopped.dir("team").join("plans"),
        target: stopped.dir("default").join("plans"),
    };
    std::fs::create_dir_all(&plans.target).unwrap();
    linker().link(std::slice::from_ref(&plans));
    stopped.synced(&["team"], 1);
    assert_eq!(read(&stopped.dir("team").join("CLAUDE.md")), "ours");
    // Even without the marker a directory of nothing but links to the default home's
    // shared directories is the agent's own …
    let links_only = World::new("profiles-links-only");
    for name in ["plans", "skills"] {
        let pair = Link {
            link: links_only.dir("team").join(name),
            target: links_only.dir("default").join(name),
        };
        std::fs::create_dir_all(&pair.target).unwrap();
        std::fs::create_dir_all(links_only.dir("team")).unwrap();
        linker().link(std::slice::from_ref(&pair));
    }
    links_only.synced(&["team"], 1);
    // … but not one whose link goes somewhere else, or that holds anything besides.
    let other = World::new("profiles-links-other");
    let elsewhere = other.home().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::create_dir_all(other.dir("team")).unwrap();
    let pair = Link {
        link: other.dir("team").join("plans"),
        target: elsewhere,
    };
    linker().link(std::slice::from_ref(&pair));
    let reports = other.sync(&["team"], 1);
    assert!(reports[0].foreign, "{:?}", reports[0].errors);
    assert!(!other.dir("team").join(MARKER).exists());
}

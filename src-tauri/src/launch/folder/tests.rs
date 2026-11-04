use super::*;

#[test]
fn folders_are_absolute_windows_paths_without_trailing_separator() {
    assert_eq!(
        normalize_folder(r"C:\Code\demo\"),
        Ok(r"C:\Code\demo".into())
    );
    assert_eq!(
        normalize_folder(" C:/Code/demo// "),
        Ok(r"C:\Code\demo".into())
    );
    assert_eq!(normalize_folder(r"C:\"), Ok(r"C:\".into()));
    assert_eq!(normalize_folder("d:/"), Ok(r"d:\".into()));
    assert_eq!(
        normalize_folder(r"\\server\share\x\"),
        Ok(r"\\server\share\x".into())
    );
    for bad in ["", "Code\\demo", "C:", "C:demo", "/home/x", r"\\"] {
        assert_eq!(
            normalize_folder(bad),
            Err("Bitte einen absoluten Ordnerpfad wählen.".into()),
            "{bad}"
        );
    }
    assert_eq!(
        normalize_folder("C:\\a\u{0}b"),
        Err("Der Ordnerpfad enthält Steuerzeichen.".into())
    );
    assert!(normalize_folder(&format!("C:\\{}", "a".repeat(1100))).is_err());
}

#[test]
fn device_stream_and_reserved_folder_forms_are_refused() {
    for device in [
        r"\\?\C:\Code",
        r"\\.\C:\Code",
        "//?/C:/Code",
        r"\\.\PhysicalDrive0",
    ] {
        assert_eq!(
            normalize_folder(device),
            Err(DEVICE_PATH.into()),
            "{device}"
        );
    }
    let invalid = [
        r"C:\a:b",
        r"C:\Code\repo::$INDEX_ALLOCATION",
        r"C:\Code:stream\x",
        r"\\server\share:x",
        r"C:\a<b",
        r"C:\a>b",
        r#"C:\a"b"#,
        r"C:\a|b",
        r"C:\a?b",
        r"C:\a*",
    ];
    for folder in invalid {
        assert_eq!(
            normalize_folder(folder),
            Err(INVALID_CHARS.into()),
            "{folder}"
        );
    }
    let reserved = [
        r"C:\CON",
        r"C:\Code\nul",
        r"C:\Code\Aux.txt",
        r"C:\prn.tar.gz\x",
        r"C:\COM1",
        r"C:\Code\lpt9.log",
        "C:\\com\u{00B9}",
        r"C:\NUL \x",
        r"C:\conout$",
        r"C:\con.d",
    ];
    for folder in reserved {
        assert_eq!(
            normalize_folder(folder),
            Err(RESERVED_NAME.into()),
            "{folder}"
        );
    }
    for fine in [
        r"C:\Console",
        r"C:\Code\nully",
        r"C:\COM10",
        r"C:\LPT",
        r"C:\Code\aux-tools",
        r"C:\COM",
        r"\\server\share\x",
        r"C:\Max's Projekt; & Co (1) 100%",
    ] {
        assert!(normalize_folder(fine).is_ok(), "{fine}");
    }
}

/// Windows strips a trailing `.`/space and resolves `.`/`..` before the folder is used, so
/// `is_dir` and docker `-v` would disagree; repeated separators collapse instead.
#[test]
fn dot_segments_and_trailing_dots_or_spaces_are_refused() {
    for folder in [
        r"C:\proj.",
        r"C:\proj. . .",
        r"C:\Code\repo \x",
        r"\\server.\share",
    ] {
        assert_eq!(
            normalize_folder(folder),
            Err(TRAILING_DOT_OR_SPACE.into()),
            "{folder}"
        );
    }
    for folder in [
        r"C:\a\..\b",
        r"C:\a\.\b",
        r"C:\..",
        "C:/a/./",
        r"\\server\..\x",
    ] {
        assert_eq!(
            normalize_folder(folder),
            Err(DOT_SEGMENT.into()),
            "{folder}"
        );
    }
    assert_eq!(normalize_folder(r"c:\a\\\b"), Ok(r"c:\a\b".into()));
    assert_eq!(normalize_folder(r"C:\\a//b\\"), Ok(r"C:\a\b".into()));
    assert_eq!(normalize_folder(r"C:\\"), Ok(r"C:\".into()));
    assert_eq!(
        normalize_folder(r"\\server\share\x"),
        Ok(r"\\server\share\x".into())
    );
    assert_eq!(
        normalize_folder(r"\\server\\share"),
        Ok(r"\\server\share".into())
    );
    for folder in [r"\\\server\share", r"\\\"] {
        assert_eq!(
            normalize_folder(folder),
            Err(NOT_ABSOLUTE.into()),
            "{folder}"
        );
    }
    for fine in [r"C:\.git-like\a.b", r"C:\a\...x", r"C:\ lead"] {
        assert!(normalize_folder(fine).is_ok(), "{fine}");
    }
}

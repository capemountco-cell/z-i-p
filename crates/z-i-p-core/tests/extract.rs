//! Unpacking real archives, built here on the fly (RAR, which cannot be
//! created freely, comes from small fixtures in tests/data).

use std::cell::Cell;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use z_i_p_core::extract::Silent;
use z_i_p_core::{extract, list, Error, Format, Progress, Request};

fn zip_file(path: &Path, entries: &[(&str, &[u8])]) {
    let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in entries {
        if name.ends_with('/') {
            w.add_directory(*name, opts).unwrap();
        } else {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
    }
    w.finish().unwrap();
}

/// Files and folders under `dir`, relative and sorted, folders ending in `/`.
fn tree(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            if p.is_dir() && !p.is_symlink() {
                out.push(format!("{rel}/"));
                walk(base, &p, out);
            } else {
                out.push(rel);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

struct Setup {
    _tmp: tempfile::TempDir,
    src: PathBuf,
    dest: PathBuf,
}

fn setup() -> Setup {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    Setup {
        src,
        dest,
        _tmp: tmp,
    }
}

#[test]
fn single_folder_unpacks_without_nesting_and_skips_clutter() {
    let s = setup();
    let a = s.src.join("Photos.zip");
    zip_file(
        &a,
        &[
            ("Photos/", b""),
            ("Photos/a.jpg", b"aaa"),
            ("Photos/sub/b.jpg", b"bbbb"),
            ("__MACOSX/Photos/._a.jpg", b"junk"),
            ("Photos/.DS_Store", b"junk"),
        ],
    );
    let out = extract(&Request::new(&a, &s.dest), &Silent).unwrap();
    assert_eq!(PathBuf::from(&out.output), s.dest.join("Photos"));
    assert_eq!(
        tree(&s.dest),
        ["Photos/", "Photos/a.jpg", "Photos/sub/", "Photos/sub/b.jpg"]
    );
    assert_eq!(out.files, 2);
    assert_eq!(out.bytes, 7);
    assert_eq!(out.skipped, 2);
}

#[test]
fn loose_files_go_into_a_folder_named_after_the_archive_and_never_overwrite() {
    let s = setup();
    let a = s.src.join("Notes.zip");
    zip_file(&a, &[("one.txt", b"1"), ("two.txt", b"2")]);
    let first = extract(&Request::new(&a, &s.dest), &Silent).unwrap();
    let second = extract(&Request::new(&a, &s.dest), &Silent).unwrap();
    assert_eq!(PathBuf::from(first.output), s.dest.join("Notes"));
    assert_eq!(PathBuf::from(second.output), s.dest.join("Notes 2"));
    assert_eq!(
        fs::read_to_string(s.dest.join("Notes 2/two.txt")).unwrap(),
        "2"
    );
    // Nothing but the two results: the staging folder is gone.
    assert_eq!(fs::read_dir(&s.dest).unwrap().count(), 2);
}

#[test]
fn names_that_escape_are_refused() {
    let s = setup();
    let a = s.src.join("evil.zip");
    zip_file(
        &a,
        &[
            ("../../escaped.txt", b"x"),
            ("ok.txt", b"fine"),
            ("/abs.txt", b"y"),
        ],
    );
    let out = extract(&Request::new(&a, &s.dest), &Silent).unwrap();
    assert_eq!(out.unsafe_names, 1);
    assert!(!s.src.parent().unwrap().join("escaped.txt").exists());
    assert_eq!(tree(&s.dest), ["evil/", "evil/abs.txt", "evil/ok.txt"]);
}

#[test]
fn picks_only_the_chosen_files() {
    let s = setup();
    let a = s.src.join("Big.zip");
    zip_file(
        &a,
        &[
            ("a/1.txt", b"1"),
            ("a/2.txt", b"2"),
            ("b/3.txt", b"3"),
            ("ab/4.txt", b"4"),
        ],
    );
    let mut req = Request::new(&a, &s.dest);
    req.only = Some(vec!["a".into(), "b/3.txt".into()]);
    extract(&req, &Silent).unwrap();
    assert_eq!(
        tree(&s.dest),
        [
            "Big/",
            "Big/a/",
            "Big/a/1.txt",
            "Big/a/2.txt",
            "Big/b/",
            "Big/b/3.txt"
        ]
    );
}

#[test]
fn encrypted_zip_asks_for_and_checks_the_password() {
    let s = setup();
    let a = s.src.join("secret.zip");
    {
        let mut w = zip::ZipWriter::new(fs::File::create(&a).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .with_aes_encryption(zip::AesMode::Aes256, "hunter2");
        w.start_file("plan.txt", opts).unwrap();
        w.write_all(b"the plan").unwrap();
        w.finish().unwrap();
    }
    let listing = list(&a, None).unwrap();
    assert!(listing.encrypted);

    assert_eq!(
        extract(&Request::new(&a, &s.dest), &Silent),
        Err(Error::PasswordRequired)
    );
    let mut req = Request::new(&a, &s.dest);
    req.password = Some("wrong".into());
    assert_eq!(extract(&req, &Silent), Err(Error::WrongPassword));
    assert_eq!(
        fs::read_dir(&s.dest).unwrap().count(),
        0,
        "a failed attempt leaves nothing behind"
    );

    req.password = Some("hunter2".into());
    let out = extract(&req, &Silent).unwrap();
    assert_eq!(fs::read_to_string(out.output).unwrap(), "the plan");
}

#[test]
fn tar_gz_keeps_permissions_and_safe_links_only() {
    let s = setup();
    let a = s.src.join("tool.tar.gz");
    {
        let gz = flate2::write::GzEncoder::new(
            fs::File::create(&a).unwrap(),
            flate2::Compression::fast(),
        );
        let mut b = tar::Builder::new(gz);
        let mut h = tar::Header::new_gnu();
        h.set_size(5);
        h.set_mode(0o755);
        h.set_cksum();
        b.append_data(&mut h, "tool/run", &b"#!/sh"[..]).unwrap();
        for (name, target) in [("tool/link", "run"), ("tool/bad", "../../etc/passwd")] {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(tar::EntryType::Symlink);
            h.set_size(0);
            b.append_link(&mut h, name, target).unwrap();
        }
        b.into_inner().unwrap().finish().unwrap();
    }
    assert_eq!(list(&a, None).unwrap().format, Format::TarGz);
    let out = extract(&Request::new(&a, &s.dest), &Silent).unwrap();
    assert_eq!(out.unsafe_names, 1);
    assert_eq!(tree(&s.dest), ["tool/", "tool/link", "tool/run"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(s.dest.join("tool/run"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
        assert_eq!(
            fs::read_link(s.dest.join("tool/link")).unwrap(),
            PathBuf::from("run")
        );
    }
}

#[test]
fn single_compressed_file_lands_next_to_it() {
    let s = setup();
    let a = s.src.join("report.csv.gz");
    let mut e =
        flate2::write::GzEncoder::new(fs::File::create(&a).unwrap(), flate2::Compression::fast());
    e.write_all(b"a,b\n1,2\n").unwrap();
    e.finish().unwrap();
    let out = extract(&Request::new(&a, &s.dest), &Silent).unwrap();
    assert_eq!(PathBuf::from(&out.output), s.dest.join("report.csv"));
    assert_eq!(fs::read_to_string(&out.output).unwrap(), "a,b\n1,2\n");
}

#[test]
fn seven_zip_plain_encrypted_and_split() {
    let s = setup();
    let content = s.src.join("content");
    fs::create_dir_all(content.join("inner")).unwrap();
    fs::write(content.join("inner/x.txt"), "xx").unwrap();
    fs::write(content.join("y.txt"), "yyy").unwrap();

    let plain = s.src.join("plain.7z");
    sevenz_rust2::compress_to_path(&content, &plain).unwrap();
    let listing = list(&plain, None).unwrap();
    assert_eq!(listing.format, Format::SevenZ);
    let out = extract(&Request::new(&plain, &s.dest), &Silent).unwrap();
    assert_eq!(
        fs::read_to_string(PathBuf::from(&out.output).join("inner/x.txt")).unwrap(),
        "xx"
    );

    let locked = s.src.join("locked.7z");
    sevenz_rust2::compress_to_path_encrypted(&content, &locked, "pw".into()).unwrap();
    assert_eq!(
        extract(&Request::new(&locked, &s.dest), &Silent),
        Err(Error::PasswordRequired)
    );
    let mut req = Request::new(&locked, &s.dest);
    req.password = Some("nope".into());
    assert_eq!(extract(&req, &Silent), Err(Error::WrongPassword));
    req.password = Some("pw".into());
    let out = extract(&req, &Silent).unwrap();
    assert_eq!(
        fs::read_to_string(PathBuf::from(&out.output).join("y.txt")).unwrap(),
        "yyy"
    );

    // The same archive cut into three pieces, opened from the second one.
    let bytes = fs::read(&plain).unwrap();
    let third = bytes.len() / 3 + 1;
    for (i, chunk) in bytes.chunks(third).enumerate() {
        fs::write(s.src.join(format!("split.7z.{:03}", i + 1)), chunk).unwrap();
    }
    let piece = s.src.join("split.7z.002");
    assert_eq!(list(&piece, None).unwrap().pieces, 3);
    let out = extract(&Request::new(&piece, &s.dest), &Silent).unwrap();
    assert_eq!(PathBuf::from(&out.output), s.dest.join("split"));
}

#[test]
fn rar_plain_and_encrypted() {
    let s = setup();
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");

    let listing = list(&data.join("version.rar"), None).unwrap();
    assert_eq!(listing.format, Format::Rar);
    let out = extract(&Request::new(data.join("version.rar"), &s.dest), &Silent).unwrap();
    assert_eq!(out.files, 1);

    let crypted = data.join("crypted.rar");
    assert!(list(&crypted, None).unwrap().encrypted);
    assert_eq!(
        extract(&Request::new(&crypted, &s.dest), &Silent),
        Err(Error::PasswordRequired)
    );
    let mut req = Request::new(&crypted, &s.dest);
    req.password = Some("unrar".into());
    let out = extract(&req, &Silent).unwrap();
    assert_eq!(
        fs::read_to_string(out.output).unwrap(),
        "target\nCargo.lock\n"
    );

    // Encrypted names: even listing needs the password.
    let hidden = data.join("comment-hpw-password.rar");
    assert_eq!(
        list(&hidden, None).map(|_| ()),
        Err(Error::PasswordRequired)
    );
    assert_eq!(
        list(&hidden, Some("password")).unwrap().entries[0].path,
        ".gitignore"
    );
}

struct StopAfterFirstReport(Cell<bool>);
impl Progress for StopAfterFirstReport {
    fn report(&self, _: u64, _: u64, _: &str) {
        self.0.set(true);
    }
    fn should_stop(&self) -> bool {
        self.0.get()
    }
}

#[test]
fn stopping_leaves_nothing_behind() {
    let s = setup();
    let a = s.src.join("big.zip");
    let big = vec![7u8; 3_000_000];
    zip_file(&a, &[("a.bin", &big), ("b.bin", &big)]);
    let r = extract(
        &Request::new(&a, &s.dest),
        &StopAfterFirstReport(Cell::new(false)),
    );
    assert_eq!(r, Err(Error::Cancelled));
    assert_eq!(fs::read_dir(&s.dest).unwrap().count(), 0);
}

#[test]
fn truncated_download_is_reported_as_damaged() {
    let s = setup();
    let a = s.src.join("cut.zip");
    zip_file(&a, &[("a.txt", b"hello hello hello")]);
    let bytes = fs::read(&a).unwrap();
    fs::write(&a, &bytes[..bytes.len() - 30]).unwrap();
    assert!(matches!(
        extract(&Request::new(&a, &s.dest), &Silent),
        Err(Error::Damaged { .. })
    ));
    assert_eq!(fs::read_dir(&s.dest).unwrap().count(), 0);
}

#[test]
fn not_an_archive() {
    let s = setup();
    let a = s.src.join("notes.txt");
    fs::write(&a, "just words").unwrap();
    assert!(matches!(list(&a, None), Err(Error::Unsupported { .. })));
    assert!(matches!(
        list(&s.src.join("missing.zip"), None),
        Err(Error::NotFound { .. })
    ));
}

# Z-I-P

Unpacks any archive on your Mac: ZIP, RAR, 7z, TAR, GZ, BZ2, XZ and ZST,
including split archives (`.part1.rar`, `.7z.001`) and password-protected
ones. Dark, quiet, and careful with your files.

Drop archives on the window (or open them from Finder) and they unpack next
to themselves. Or look inside first and take out only the files you want.

**Careful with your files**

- Your archive is never changed or deleted.
- Nothing is overwritten: if `Photos` exists you get `Photos 2`.
- Files appear only once every one is out. A failed or stopped unpack
  leaves nothing behind.
- Free space is checked before the first byte is written.
- Names that point outside the destination (`../../`) are refused.
- An archive holding one folder unpacks to that folder, not `Photos/Photos`.
- `__MACOSX`, `.DS_Store` and `._` files are left out.
- Japanese, Chinese and other non-Latin names from old Windows zips are
  decoded correctly.
- Files from a downloaded archive keep macOS's "downloaded" mark, so
  Gatekeeper checks apps inside it.
- Passwords are kept in memory for the session only and tried across the
  rest of a batch, so you type one once. No network connections.

See [docs/product.md](docs/product.md) for what users of other unarchivers
asked for, and the order of work.

Built with Tauri 2, Rust and React, from the Mac App Template. The look
follows Camu's.

## Where things live

| Path | What |
|---|---|
| `crates/z-i-p-core/src/format.rs` | Recognising an archive from its first bytes; split pieces |
| `crates/z-i-p-core/src/list.rs` | Listing contents without unpacking |
| `crates/z-i-p-core/src/extract.rs` | Unpacking safely (staging, no overwrite, space check) |
| `crates/z-i-p-core/src/names.rs` | Safe paths, name encodings, clutter, unique names |
| `crates/z-i-p-core/tests/` | Real archives built and unpacked, plus RAR fixtures |
| `src-tauri/src/lib.rs` | Thin IPC commands, progress events, Finder "Open With" |
| `src/` | The UI. Every call to Rust goes through `src/lib/native.ts` |

## Everyday commands

| Command | Does |
| --- | --- |
| `npm run dev` | Runs the app with live reload |
| `npm run dev:web` | The interface in a browser; add `?demo` for sample data |
| `npm run check` | Type check, tests, Rust format, lint and tests. Run before every push |
| `npm run dmg` | Builds and verifies a `.dmg` for this Mac's chip |
| `npm run dmg:universal` | One `.dmg` for Apple silicon and Intel |
| `npm run icons` | Regenerates every icon size from `src-tauri/app-icon.png` (1024×1024) |
| `npm run set-version -- 1.2.0` | Sets the version everywhere it lives |
| `npm run doctor` | Lists anything missing on this machine, with the fix |

## Releasing

```sh
npm run set-version -- 1.0.0
git commit -am "Release 1.0.0"
git tag v1.0.0
git push && git push --tags
```

GitHub Actions builds a universal `.dmg` and attaches it to a GitHub release
([docs/signing.md](docs/signing.md) for signing and notarizing).

## Licences

RAR support uses the unRAR source code by Alexander Roshal (via the `unrar`
crate), whose licence allows extracting RAR archives but not creating them.

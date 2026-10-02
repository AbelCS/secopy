# Changelog

## [0.22.1](https://github.com/AbelCS/secopy/compare/v0.22.0...v0.22.1) (2026-10-02)


### Bug Fixes

* **ui:** New copy doesn't flash on option changes; Also ignore next to the options ([28a0877](https://github.com/AbelCS/secopy/commit/28a0877f1b779a99bc65f02e6c2b3dee035baf67)), closes [#167](https://github.com/AbelCS/secopy/issues/167)
* **ui:** review fixes for New copy's quick rescans ([#167](https://github.com/AbelCS/secopy/issues/167)) ([65c6560](https://github.com/AbelCS/secopy/commit/65c6560e2a5f907cd5b3859235bed13c56723031))

## [0.22.0](https://github.com/AbelCS/secopy/compare/v0.21.1...v0.22.0) (2026-10-02)


### Features

* **app:** New copy's own ignore list ([#164](https://github.com/AbelCS/secopy/issues/164)) ([f4bad0e](https://github.com/AbelCS/secopy/commit/f4bad0ec9adf35a93773a9bbce482e60887a9f6b))
* **app:** presets and queued jobs keep their own ignore list ([#164](https://github.com/AbelCS/secopy/issues/164)) ([963b17c](https://github.com/AbelCS/secopy/commit/963b17c19ebff87fc82259e44830c4f7e3d20934))
* **app:** queued jobs and mirrors use their own ignore list ([#164](https://github.com/AbelCS/secopy/issues/164)) ([4dab9e1](https://github.com/AbelCS/secopy/commit/4dab9e1550ff91cd3ba1e137ff6c37869b2b8bcd))
* **core:** ignore lists hold up to 128, and a job's adds to the global one ([#164](https://github.com/AbelCS/secopy/issues/164)) ([e954791](https://github.com/AbelCS/secopy/commit/e9547919b3599ad307750f3baafc359b58872e94))
* **ui:** Also ignore in New copy and the preset editors ([#164](https://github.com/AbelCS/secopy/issues/164)) ([0995cbd](https://github.com/AbelCS/secopy/commit/0995cbd9c473012aa248dd7c7df5bf229be5d9b8))


### Bug Fixes

* review fixes for each copy and preset's own ignore list ([#164](https://github.com/AbelCS/secopy/issues/164)) ([00da3bb](https://github.com/AbelCS/secopy/commit/00da3bb2641b68d57d0b13529f7630469d48599d))

## [0.21.1](https://github.com/AbelCS/secopy/compare/v0.21.0...v0.21.1) (2026-10-01)


### Bug Fixes

* review fixes for the ignore list's layout ([#161](https://github.com/AbelCS/secopy/issues/161)) ([eb87b13](https://github.com/AbelCS/secopy/commit/eb87b134d16f581e99cfd8e09e4632c10c3bec0b))
* **ui:** the ignore list as a readable, scrolling list ([c19a376](https://github.com/AbelCS/secopy/commit/c19a376e62851bd58407c19e4c5668687d8ecf2e)), closes [#161](https://github.com/AbelCS/secopy/issues/161)

## [0.21.0](https://github.com/AbelCS/secopy/compare/v0.20.0...v0.21.0) (2026-10-01)


### Features

* **app:** the Always ignore list, saved and used by every scan ([#158](https://github.com/AbelCS/secopy/issues/158)) ([9ea3673](https://github.com/AbelCS/secopy/commit/9ea3673b55bb1e4b457a5d828c9193473a7308e1))
* **cli:** --ignore ([#158](https://github.com/AbelCS/secopy/issues/158)) ([7b204d0](https://github.com/AbelCS/secopy/commit/7b204d037ecf1a2f959b2a338cefdc6555d0d597))
* **core:** ASC MHL histories ignore the list ([#158](https://github.com/AbelCS/secopy/issues/158)) ([0e59029](https://github.com/AbelCS/secopy/commit/0e59029454c995ec07c06648368a5c5ee934710c))
* **core:** ignore patterns ([#158](https://github.com/AbelCS/secopy/issues/158)) ([53c41dd](https://github.com/AbelCS/secopy/commit/53c41dd0e52917b5a5fd68a733858ce45aa8770d))
* **core:** mirrors leave ignored files alone ([#158](https://github.com/AbelCS/secopy/issues/158)) ([2cb67d5](https://github.com/AbelCS/secopy/commit/2cb67d5d0c6695471ff150988fa72472f270fb30))
* **core:** scans skip what the ignore list names ([#158](https://github.com/AbelCS/secopy/issues/158)) ([f05f428](https://github.com/AbelCS/secopy/commit/f05f428e52fa7ce700c5673df8773f4fd0b0e8d1))
* **core:** Verify follows the ignore list ([#158](https://github.com/AbelCS/secopy/issues/158)) ([0e2993c](https://github.com/AbelCS/secopy/commit/0e2993cccd1570009744bca11a0a0835b9ab6898))
* **ui:** Always ignore when copying in Settings ([#158](https://github.com/AbelCS/secopy/issues/158)) ([abd94eb](https://github.com/AbelCS/secopy/commit/abd94ebde8e397f69d63afb705eb370b4dd92e47))


### Bug Fixes

* review fixes for the ignore list ([#158](https://github.com/AbelCS/secopy/issues/158)) ([1b55e6a](https://github.com/AbelCS/secopy/commit/1b55e6ab7bec5ae68651f211d83af0259113eb9e))

## [0.20.0](https://github.com/AbelCS/secopy/compare/v0.19.0...v0.20.0) (2026-10-01)


### Features

* **app:** copies record ASC MHL and say so ([#154](https://github.com/AbelCS/secopy/issues/154)) ([827ffe7](https://github.com/AbelCS/secopy/commit/827ffe7ca2579e9542249106569ebf6f1618e6c0))
* **app:** Write ASC MHL setting and the copy's plan ([#154](https://github.com/AbelCS/secopy/issues/154)) ([61ddc5f](https://github.com/AbelCS/secopy/commit/61ddc5fc40ec0b6ca02b3ce0f27809a642e69776))
* **cli:** --mhl writes ASC MHL ([#154](https://github.com/AbelCS/secopy/issues/154)) ([46a4a83](https://github.com/AbelCS/secopy/commit/46a4a83502553b57b13797d260937d4355353319))
* **core:** ASC MHL ignore patterns ([#154](https://github.com/AbelCS/secopy/issues/154)) ([cf588f1](https://github.com/AbelCS/secopy/commit/cf588f13807a96310948fe1da206670ea9a9085d))
* **core:** C4 IDs for ASC MHL ([#154](https://github.com/AbelCS/secopy/issues/154)) ([6e999f3](https://github.com/AbelCS/secopy/commit/6e999f325cc5abadc83eccedb46d38609ded903e))
* **core:** prepare a copy's ASC MHL: scopes, files to read, blockers ([#154](https://github.com/AbelCS/secopy/issues/154)) ([768582d](https://github.com/AbelCS/secopy/commit/768582d5e555056bb3f4655807c3fc7c154f01f3))
* **core:** read and check ASC MHL histories ([#154](https://github.com/AbelCS/secopy/issues/154)) ([156cbcb](https://github.com/AbelCS/secopy/commit/156cbcb618eae99ec52104ac9457f6fbd3cf5cb2))
* **core:** record ASC MHL at the end of a copy ([#154](https://github.com/AbelCS/secopy/issues/154)) ([f48a17e](https://github.com/AbelCS/secopy/commit/f48a17e53866e342952c97878c208e6643aa77d9))
* **core:** undo takes back ASC MHL; Verify doesn't count it ([#154](https://github.com/AbelCS/secopy/issues/154)) ([9b0d944](https://github.com/AbelCS/secopy/commit/9b0d944a39cef55b33a632e8314538cc8f882b6c))
* **core:** write ASC MHL manifests and chains ([#154](https://github.com/AbelCS/secopy/issues/154)) ([ec5f01a](https://github.com/AbelCS/secopy/commit/ec5f01af6e56081151bc4d90d128a22f0f6bd213))
* **ui:** Write ASC MHL in Settings, the plan, progress and summary ([#154](https://github.com/AbelCS/secopy/issues/154)) ([1555d62](https://github.com/AbelCS/secopy/commit/1555d627b8fe3fe7bdf91b4b0d2c2c2b80b952dd))


### Bug Fixes

* review fixes for ASC MHL ([#154](https://github.com/AbelCS/secopy/issues/154)) ([f1b78b8](https://github.com/AbelCS/secopy/commit/f1b78b84bee4fb258d0208465f99d9139fcf64c0))

## [0.19.0](https://github.com/AbelCS/secopy/compare/v0.18.0...v0.19.0) (2026-10-01)


### Features

* **app:** Include the directory off by default ([949dc1d](https://github.com/AbelCS/secopy/commit/949dc1d145d4304f7e1a0768e14b738a94d26b70)), closes [#152](https://github.com/AbelCS/secopy/issues/152)

## [0.18.0](https://github.com/AbelCS/secopy/compare/v0.17.6...v0.18.0) (2026-09-30)


### Features

* **app:** import says what differs between Secopy versions ([c86b56b](https://github.com/AbelCS/secopy/commit/c86b56b648727fdae641ee927d6997457a970519)), closes [#149](https://github.com/AbelCS/secopy/issues/149)


### Bug Fixes

* **app:** review fixes for import across versions ([88bb4c0](https://github.com/AbelCS/secopy/commit/88bb4c0fef79da278c0be43ec66ce2c9fddcdcee))

## [0.17.6](https://github.com/AbelCS/secopy/compare/v0.17.5...v0.17.6) (2026-09-30)


### Bug Fixes

* **app:** a file from a newer Secopy is left as it is, not set aside ([46e2f46](https://github.com/AbelCS/secopy/commit/46e2f461503ff12306e1d8b6e848544b8178a78e)), closes [#137](https://github.com/AbelCS/secopy/issues/137)
* **app:** a queued copy with settings this version doesn't know is kept whole ([a06eab0](https://github.com/AbelCS/secopy/commit/a06eab069ef50e911bc55ef8f0791ece293d222f)), closes [#137](https://github.com/AbelCS/secopy/issues/137)
* **app:** an archive is deleted without holding the lock the window's commands take ([bee433c](https://github.com/AbelCS/secopy/commit/bee433c03dd22ba4f62e8f0ab4930a42b344424f)), closes [#134](https://github.com/AbelCS/secopy/issues/134)
* **app:** everyone waiting for a job waits for its end ([527f083](https://github.com/AbelCS/secopy/commit/527f083f05587a10012845a9076e89c81743ba4e)), closes [#134](https://github.com/AbelCS/secopy/issues/134)
* **app:** nothing starts or is queued while a new pick is scanned ([6138e21](https://github.com/AbelCS/secopy/commit/6138e210b3051a002113a05010df7d6924bb36b2)), closes [#137](https://github.com/AbelCS/secopy/issues/137)
* **app:** review fixes for the mirror's preparing step, job threads and undo ([8c92403](https://github.com/AbelCS/secopy/commit/8c924032f434c2bb8ac2e07666f81636381b64d3)), closes [#134](https://github.com/AbelCS/secopy/issues/134)
* **app:** Start and Verify's Start check and start under the queue's lock ([3f79c44](https://github.com/AbelCS/secopy/commit/3f79c44baff9161b3033bdf85fc9a91ee8c79da5)), closes [#134](https://github.com/AbelCS/secopy/issues/134)
* **app:** the Mac stays awake for a whole job and a preview's comparison ([16f360c](https://github.com/AbelCS/secopy/commit/16f360c3a362a2a04808d71ea325d4597c00e14f)), closes [#134](https://github.com/AbelCS/secopy/issues/134)
* **cli:** a mirror's dry run counts files that will fail apart ([3f22019](https://github.com/AbelCS/secopy/commit/3f22019a91201eec4ed340dae881dd82fd928c6b)), closes [#137](https://github.com/AbelCS/secopy/issues/137)
* **core:** a mirror that doesn't fit is refused before it starts ([fb074e4](https://github.com/AbelCS/secopy/commit/fb074e45215f58ca08440c24b622f5afc17c5556)), closes [#135](https://github.com/AbelCS/secopy/issues/135)
* **core:** a removed file never replaces one already archived ([6d7229d](https://github.com/AbelCS/secopy/commit/6d7229d610ac7fe1d4601f977a0244c1b2f7ff97)), closes [#136](https://github.com/AbelCS/secopy/issues/136)
* **core:** an archive run's name holds its time zone ([de67a25](https://github.com/AbelCS/secopy/commit/de67a25f21f0dc2be8eb0bb828144244e5e3cf82)), closes [#136](https://github.com/AbelCS/secopy/issues/136)
* **core:** review fixes for the archive clean-up, names and moves ([4c8ebae](https://github.com/AbelCS/secopy/commit/4c8ebae60d9d4aebc2eb388882ba34a4a694e836)), closes [#136](https://github.com/AbelCS/secopy/issues/136)
* **core:** the report says when no checksum file was written because nothing was copied ([b6934c2](https://github.com/AbelCS/secopy/commit/b6934c22d94359e3e81535a42dd5f0fd67f5fef0)), closes [#135](https://github.com/AbelCS/secopy/issues/135)
* **core:** the space check counts each file in whole allocation blocks ([a700c91](https://github.com/AbelCS/secopy/commit/a700c9117c2f0b2fc1461df8c38ea1f897d490af)), closes [#135](https://github.com/AbelCS/secopy/issues/135)
* expired archived files that can't be removed are said, and the run isn't Complete ([82e2230](https://github.com/AbelCS/secopy/commit/82e22306eee7b4d846db795f7767d869ff1fab29)), closes [#136](https://github.com/AbelCS/secopy/issues/136)
* special files are listed as skipped, not skipped without a word ([63a00e7](https://github.com/AbelCS/secopy/commit/63a00e706c15f1506375b676fb00c925795629a7)), closes [#135](https://github.com/AbelCS/secopy/issues/135)
* **ui:** a job added from New copy isn't added again if clearing after it failed ([6e3fee9](https://github.com/AbelCS/secopy/commit/6e3fee9c88a62de0120636a8aad3f6dd605372b0)), closes [#138](https://github.com/AbelCS/secopy/issues/138)
* **ui:** New copy shows the newest view, not the last answer to arrive ([87b9dd6](https://github.com/AbelCS/secopy/commit/87b9dd6a191e53f515965d1f8648ce78fddd0a13)), closes [#138](https://github.com/AbelCS/secopy/issues/138)
* **ui:** review fixes for New copy's newest view and Add to queue ([63e88bc](https://github.com/AbelCS/secopy/commit/63e88bc2b954f704f40c3113471ae45d5c0b3629)), closes [#138](https://github.com/AbelCS/secopy/issues/138)
* undo says when it can't remove the checksum file ([e651891](https://github.com/AbelCS/secopy/commit/e651891af7a7280178e5ee56cc3919a24b8fc71d)), closes [#134](https://github.com/AbelCS/secopy/issues/134)

## [0.17.5](https://github.com/AbelCS/secopy/compare/v0.17.4...v0.17.5) (2026-09-30)


### Bug Fixes

* **app:** quitting after Cancel and remove still removes the copies ([a1ba2a0](https://github.com/AbelCS/secopy/commit/a1ba2a05c26afe5b323f86c8e9c490f3db3ae9e1)), closes [#118](https://github.com/AbelCS/secopy/issues/118)
* **core:** a copy gets only its owner's read bit added; read-only stays read-only ([1eb9bc1](https://github.com/AbelCS/secopy/commit/1eb9bc12b728252f2a0a9050df9bd4316cc1bbe4)), closes [#118](https://github.com/AbelCS/secopy/issues/118)
* **core:** a copy stays readable and writable by its owner ([2171148](https://github.com/AbelCS/secopy/commit/2171148ba6d6cd604c0abadc8882236a0a8e0c30)), closes [#118](https://github.com/AbelCS/secopy/issues/118)
* **core:** a small file that grows while it's read isn't read into memory without end ([3fbc0d0](https://github.com/AbelCS/secopy/commit/3fbc0d08fded1a207f65c50deb45e4b4073b4a4d)), closes [#118](https://github.com/AbelCS/secopy/issues/118)

## [0.17.4](https://github.com/AbelCS/secopy/compare/v0.17.3...v0.17.4) (2026-09-30)


### Bug Fixes

* **ui:** a double click doesn't remove two queued jobs or add one twice ([9dbc3bc](https://github.com/AbelCS/secopy/commit/9dbc3bc82ac2db59294edf09f5e7b0ba5a924a93)), closes [#117](https://github.com/AbelCS/secopy/issues/117)
* **ui:** a job shows 0 %, not 100 %, before its first figures ([1d280e0](https://github.com/AbelCS/secopy/commit/1d280e0a670252761882ffb5a133982d83bf2ed9)), closes [#117](https://github.com/AbelCS/secopy/issues/117)
* **ui:** a page of the finished list that failed to load is asked for again ([3f98888](https://github.com/AbelCS/secopy/commit/3f9888873d28f4ba5fb37e1e5291f591e4713c4a)), closes [#117](https://github.com/AbelCS/secopy/issues/117)
* **ui:** a summary that can't be loaded goes back to the job's section with why ([88e9436](https://github.com/AbelCS/secopy/commit/88e94369ed6da1707923d995e3442dcb684d6177)), closes [#117](https://github.com/AbelCS/secopy/issues/117)
* **ui:** review fixes for the queue, Verify, a preview's end and the finished list ([e129b27](https://github.com/AbelCS/secopy/commit/e129b27ff8377e513addc5cec18d38a5470c7800)), closes [#117](https://github.com/AbelCS/secopy/issues/117)
* **ui:** Tab stays inside a dialog ([bb04d80](https://github.com/AbelCS/secopy/commit/bb04d800dbf8cce5ab7ffd2075c76be0f005770d)), closes [#117](https://github.com/AbelCS/secopy/issues/117)
* **ui:** the menu bar panel never shows an older view over a newer one ([3fa5a53](https://github.com/AbelCS/secopy/commit/3fa5a53900d3afe334b7d68e7970c215eb9b03e8)), closes [#117](https://github.com/AbelCS/secopy/issues/117)
* **ui:** unsaved mirror edits are asked about before leaving for a tab, the menu or a preview ([8e474f0](https://github.com/AbelCS/secopy/commit/8e474f09971466e30d76fd3ff0c70c333054780c)), closes [#117](https://github.com/AbelCS/secopy/issues/117)

## [0.17.3](https://github.com/AbelCS/secopy/compare/v0.17.2...v0.17.3) (2026-09-30)


### Bug Fixes

* **app:** importing over the selected copy preset loads the imported one ([5e743fb](https://github.com/AbelCS/secopy/commit/5e743fb45e567656e53e17a6b4d390fc0d69da7e)), closes [#116](https://github.com/AbelCS/secopy/issues/116)
* **app:** Save report… never replaces a file of the user's with its JSON ([6631f0b](https://github.com/AbelCS/secopy/commit/6631f0b6917536b152f538c8e8850d6b31294d3a)), closes [#116](https://github.com/AbelCS/secopy/issues/116)
* **app:** settings that can't be saved aren't used either ([931d9df](https://github.com/AbelCS/secopy/commit/931d9df3819e3b1a1be0d17f9eedaf944d1c85b2)), closes [#116](https://github.com/AbelCS/secopy/issues/116)
* **cli:** nothing to copy is an error, and --report works for a mirror ([2375a62](https://github.com/AbelCS/secopy/commit/2375a62850f81902fcc203c8c770b5be477b6124)), closes [#116](https://github.com/AbelCS/secopy/issues/116)
* **core:** a report whose name is taken gets a free one, not lost ([82e8db6](https://github.com/AbelCS/secopy/commit/82e8db618aee12c8bf57c7141695a399c69d99c2)), closes [#116](https://github.com/AbelCS/secopy/issues/116)
* review fixes for reports, the CLI and the queue notification ([a5fa92d](https://github.com/AbelCS/secopy/commit/a5fa92d12649392775fc8896cdd588cc8d33b248)), closes [#116](https://github.com/AbelCS/secopy/issues/116)
* **ui:** a job's notification says when its report couldn't be saved ([4327e66](https://github.com/AbelCS/secopy/commit/4327e662e23c57efa071e17e67514493bedb8e96)), closes [#116](https://github.com/AbelCS/secopy/issues/116)
* **ui:** a queue says every job finished only when each is complete and it was saved ([890f67f](https://github.com/AbelCS/secopy/commit/890f67f6c2da7a40628c8dc50ad09309299c9442)), closes [#116](https://github.com/AbelCS/secopy/issues/116)

## [0.17.2](https://github.com/AbelCS/secopy/compare/v0.17.1...v0.17.2) (2026-09-30)


### Bug Fixes

* **core:** a destination gone before the final flush is a durability error ([e38e1ab](https://github.com/AbelCS/secopy/commit/e38e1abfdb5d728bf66cf63f246ccade8b8a7bb9)), closes [#115](https://github.com/AbelCS/secopy/issues/115)
* **core:** a picked file named like an unfinished copy is skipped as Secopy's own ([901adf6](https://github.com/AbelCS/secopy/commit/901adf63d68737a116aae674f88bad820034994e)), closes [#115](https://github.com/AbelCS/secopy/issues/115)
* **core:** a replaced file's old version that can't be put back is named ([25af787](https://github.com/AbelCS/secopy/commit/25af7874bec4ddf6bb11a8eed6ebd80b54375f02)), closes [#115](https://github.com/AbelCS/secopy/issues/115)
* **core:** every folder on the way to a file gets its date back and is made durable ([66616c0](https://github.com/AbelCS/secopy/commit/66616c03edf03d29114c50f5dcecd922b0713af9)), closes [#115](https://github.com/AbelCS/secopy/issues/115)
* **core:** review fixes for undo and durability ([7d0d4b0](https://github.com/AbelCS/secopy/commit/7d0d4b088d4a9ea090261861eb3477d8792a9947)), closes [#115](https://github.com/AbelCS/secopy/issues/115)
* **core:** undo removes a copy only while it's the same file (device and inode) ([2dfc3b5](https://github.com/AbelCS/secopy/commit/2dfc3b57e9b92f44dbeb0d22f060930a384edd3d)), closes [#115](https://github.com/AbelCS/secopy/issues/115)

## [0.17.1](https://github.com/AbelCS/secopy/compare/v0.17.0...v0.17.1) (2026-09-30)


### Bug Fixes

* **app:** a pending archive deletion ends with Archive mode, and a day is the least ([8e7cf5b](https://github.com/AbelCS/secopy/commit/8e7cf5b8d14ef75d00f1827f363ec70422136433)), closes [#113](https://github.com/AbelCS/secopy/issues/113)
* **core:** a destination directory that can't be read stops the mirror's removals ([0a2f008](https://github.com/AbelCS/secopy/commit/0a2f00833880d1d91cf49035718af6defa2d4704)), closes [#114](https://github.com/AbelCS/secopy/issues/114)
* **core:** a mirror renames a directory spelled otherwise, once ([51247e7](https://github.com/AbelCS/secopy/commit/51247e725244667d2a429c42d6291e992c153691)), closes [#114](https://github.com/AbelCS/secopy/issues/114)
* **core:** a removal that can't be looked at is a failure, not "already gone" ([7ac0fe3](https://github.com/AbelCS/secopy/commit/7ac0fe339586e2fa9227de2c59e1a8116a4e80f4)), closes [#114](https://github.com/AbelCS/secopy/issues/114)
* **core:** files under an origin link are never removed from the backup ([e178b49](https://github.com/AbelCS/secopy/commit/e178b4988f3c7af473f37dacd60e0bac0e3b1dd4)), closes [#114](https://github.com/AbelCS/secopy/issues/114)
* **core:** review fixes for the mirror's checksum file and renames ([c147766](https://github.com/AbelCS/secopy/commit/c147766fd88e6670a16c2d4152ef13a9004d1c91)), closes [#114](https://github.com/AbelCS/secopy/issues/114)
* **core:** the mirror's checksum file records what a run verified, even if it wasn't clean ([cba303f](https://github.com/AbelCS/secopy/commit/cba303f1b3aca7a1c89a84445ab8bb785f164aff)), closes [#114](https://github.com/AbelCS/secopy/issues/114)
* **ui:** Delete archive… deletes the archive it showed, and only after the edit is saved ([bdb2d23](https://github.com/AbelCS/secopy/commit/bdb2d2356d25c2ae733c483ce150111094c67e76)), closes [#113](https://github.com/AbelCS/secopy/issues/113)
* **ui:** importing a mirror with fewer days says what its next run removes ([5acf261](https://github.com/AbelCS/secopy/commit/5acf26102d0bc2c5f164fc0fdebb5d0e76cb08c6)), closes [#113](https://github.com/AbelCS/secopy/issues/113)
* **ui:** questions before risky actions answer no on Return and Esc ([1d95549](https://github.com/AbelCS/secopy/commit/1d95549addc84465a07690a59d017d5bc9398151)), closes [#113](https://github.com/AbelCS/secopy/issues/113)
* **ui:** review fixes for the confirmations and the mirror archive ([6b2e598](https://github.com/AbelCS/secopy/commit/6b2e598bd314e5aa928f5e8b5d8e17ad8a7c8c6a)), closes [#113](https://github.com/AbelCS/secopy/issues/113)

## [0.17.0](https://github.com/AbelCS/secopy/compare/v0.16.1...v0.17.0) (2026-09-30)


### Features

* **ui:** Start's status says how many files it replaces ([b7252f6](https://github.com/AbelCS/secopy/commit/b7252f6638dc6e53e7f7b935b039207e993320d1)), closes [#112](https://github.com/AbelCS/secopy/issues/112)


### Bug Fixes

* **app:** a queued copy overwrites only the files shown when it was queued ([8b3830e](https://github.com/AbelCS/secopy/commit/8b3830ec9fed033688378f51ee866513eac0f183)), closes [#112](https://github.com/AbelCS/secopy/issues/112)
* **app:** Start also refuses when a file to overwrite was rewritten since ([b45ad06](https://github.com/AbelCS/secopy/commit/b45ad0629a4aa5b5e8a76aa90e9ea0f51456cb69)), closes [#112](https://github.com/AbelCS/secopy/issues/112)
* **app:** Start checks the destination again and refuses if it changed ([688f810](https://github.com/AbelCS/secopy/commit/688f8100807e464b28ffe636d3fc23782cf19fe0)), closes [#112](https://github.com/AbelCS/secopy/issues/112)
* **app:** the choice for files that differ resets when what is copied changes ([a320018](https://github.com/AbelCS/secopy/commit/a320018c9ceecadbf7e039a040c7d9be085fb873)), closes [#112](https://github.com/AbelCS/secopy/issues/112)
* **core:** a source file is never a copy's target ([95e3faf](https://github.com/AbelCS/secopy/commit/95e3fafd1cc521b9e13d0e91ad762fb2a024403b)), closes [#112](https://github.com/AbelCS/secopy/issues/112)
* **core:** names that differ only in Unicode form clash ([ddd5006](https://github.com/AbelCS/secopy/commit/ddd50064086d36481a616b3f599c5938f51acaad)), closes [#112](https://github.com/AbelCS/secopy/issues/112)

## [0.16.1](https://github.com/AbelCS/secopy/compare/v0.16.0...v0.16.1) (2026-09-29)


### Bug Fixes

* **core:** free space counts purgeable space, and warns when a copy needs it ([d61ad60](https://github.com/AbelCS/secopy/commit/d61ad609728c3078aa24af5727c39b8f5d0d7e5f)), closes [#108](https://github.com/AbelCS/secopy/issues/108)
* **ui:** "Files go to" shows even when Start is blocked ([c30b38b](https://github.com/AbelCS/secopy/commit/c30b38b10b9a89c6e907f689e754706be0e28ecd)), closes [#109](https://github.com/AbelCS/secopy/issues/109)

## [0.16.0](https://github.com/AbelCS/secopy/compare/v0.15.0...v0.16.0) (2026-09-29)


### Features

* **ui:** a mirror's archive on its screen ([c83fb99](https://github.com/AbelCS/secopy/commit/c83fb9970c27b8699e79c80a3f7a1b26d75b40b2)), closes [#99](https://github.com/AbelCS/secopy/issues/99)
* **ui:** mirror comparison: Standard or Paranoid ([e59afad](https://github.com/AbelCS/secopy/commit/e59afade26a28a32eedba669d837dd0c66d5bec0)), closes [#100](https://github.com/AbelCS/secopy/issues/100)

## [0.15.0](https://github.com/AbelCS/secopy/compare/v0.14.0...v0.15.0) (2026-09-29)


### Features

* **app:** switching a mirror to Delete asks what to do with its archive ([83fbe87](https://github.com/AbelCS/secopy/commit/83fbe87892884f64f5f63dd179a40b6530bfdfea)), closes [#101](https://github.com/AbelCS/secopy/issues/101)
* **core:** say what a mirror's archive holds, and delete it ([073b990](https://github.com/AbelCS/secopy/commit/073b990d5972d7a21ffa57c307245617e82d798c)), closes [#101](https://github.com/AbelCS/secopy/issues/101)


### Bug Fixes

* **app:** the archive deletion waits for the cancel check, and an unreadable archive fails the run ([45b60a9](https://github.com/AbelCS/secopy/commit/45b60a9959c75b634dbb93f1344aee6a103e5b67)), closes [#101](https://github.com/AbelCS/secopy/issues/101)
* **app:** the archive is deleted only where, when and as often as asked ([e5e9813](https://github.com/AbelCS/secopy/commit/e5e981338c22e68a9a768fce3ecb04d7623863cf)), closes [#101](https://github.com/AbelCS/secopy/issues/101)

## [0.14.0](https://github.com/AbelCS/secopy/compare/v0.13.1...v0.14.0) (2026-09-29)


### Features

* **app:** commands, views and the saved queue speak in messages ([6050658](https://github.com/AbelCS/secopy/commit/60506582e93010850941b48641ce9cccf334f57a)), closes [#84](https://github.com/AbelCS/secopy/issues/84)
* **app:** engine outcomes as messages ([652b9a9](https://github.com/AbelCS/secopy/commit/652b9a9738f147b33fb6904dd7ed327ac4adba68)), closes [#84](https://github.com/AbelCS/secopy/issues/84)
* **app:** messages as codes: Message, msg! and say() ([8301a1e](https://github.com/AbelCS/secopy/commit/8301a1eca91b7d5d8fbeec7047af6dd555e11715)), closes [#84](https://github.com/AbelCS/secopy/issues/84)
* **app:** native menus, the menu bar icon and the source panel in the app's language ([5af7248](https://github.com/AbelCS/secopy/commit/5af724887e58e430832e795e04a05f69f162e4ec)), closes [#84](https://github.com/AbelCS/secopy/issues/84)
* **ui:** a message catalog and t() ([2e32d47](https://github.com/AbelCS/secopy/commit/2e32d47ad4b63082d016a53c5223b6ebd3af68e7)), closes [#84](https://github.com/AbelCS/secopy/issues/84)
* **ui:** format numbers and sizes for the current locale ([1810c23](https://github.com/AbelCS/secopy/commit/1810c2368174ca254b0f54088c7a6f84084c6f74)), closes [#84](https://github.com/AbelCS/secopy/issues/84)


### Bug Fixes

* **app:** the review's findings on Rust's messages ([72aa7f7](https://github.com/AbelCS/secopy/commit/72aa7f798aeff6b224d3738a9b6cebdaa9b6d1ea)), closes [#84](https://github.com/AbelCS/secopy/issues/84)
* **ui:** a preset with no name in an imported file says so ([9fdfab9](https://github.com/AbelCS/secopy/commit/9fdfab91bb76d7b124c378945e8d4f8daae8972d)), closes [#84](https://github.com/AbelCS/secopy/issues/84)
* **ui:** the last written words move to the catalog; halves round as before ([a88ddb5](https://github.com/AbelCS/secopy/commit/a88ddb568287a1628df51c1e3196089a9fa8a473)), closes [#84](https://github.com/AbelCS/secopy/issues/84)
* **ui:** the review's small issues ([0c7ec2b](https://github.com/AbelCS/secopy/commit/0c7ec2b1993d80745f95c2f4f22efd84aecdef90)), closes [#84](https://github.com/AbelCS/secopy/issues/84)

## [0.13.1](https://github.com/AbelCS/secopy/compare/v0.13.0...v0.13.1) (2026-09-29)


### Bug Fixes

* Import… waits for any job, and the refusal says "job" ([4c7fd5f](https://github.com/AbelCS/secopy/commit/4c7fd5f34c4e23e76ab2da5318a881de62d800d8)), closes [#83](https://github.com/AbelCS/secopy/issues/83)

## [0.13.0](https://github.com/AbelCS/secopy/compare/v0.12.0...v0.13.0) (2026-09-29)


### Features

* a setting to keep copying in the menu bar ([53d3f6c](https://github.com/AbelCS/secopy/commit/53d3f6c0201047254eb76ddb2c675e3761a5c606)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* **app:** keep copying in the menu bar when the window is closed ([160921f](https://github.com/AbelCS/secopy/commit/160921f2454d85fa58b541d928859ae5f3a3bbf2)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* **app:** what the menu bar icon says, and when ([96ad4c1](https://github.com/AbelCS/secopy/commit/96ad4c14a1a1c3930702303ace8a19575a882380)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* the menu bar icon opens a panel instead of a menu ([79b2a69](https://github.com/AbelCS/secopy/commit/79b2a69bc39de4474556ee006d4280db868a56f5)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* **ui:** closing during a job hides to the menu bar ([e94386f](https://github.com/AbelCS/secopy/commit/e94386fbfce11854fb7b1c94826270e7f95599c6)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* **ui:** the menu bar panel has rounded corners, like a native popover ([390bc15](https://github.com/AbelCS/secopy/commit/390bc1581dfbfd95addcbf0791169433c7c902be)), closes [#80](https://github.com/AbelCS/secopy/issues/80)


### Bug Fixes

* **app:** closing Secopy's window quits even with the panel's window ([1060946](https://github.com/AbelCS/secopy/commit/106094602a7841ac159f543a46fc7f28c8dd061b)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* **app:** the menu bar icon starts true, and a cancelled queue says so ([e610652](https://github.com/AbelCS/secopy/commit/e6106523b354dbde1d890999f30a75c5c04dd76e)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* **app:** the menu bar's menu stays open while the job runs ([de89177](https://github.com/AbelCS/secopy/commit/de89177fa5af393927fbd9c07064931df9e473a0)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* findings from reviewing the menu bar ([fbde483](https://github.com/AbelCS/secopy/commit/fbde4830f9c5c07f2ca947f1ca5748a5f2219115)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* the menu bar panel's corners are really see-through ([897a8b8](https://github.com/AbelCS/secopy/commit/897a8b8b5a1ca49167a529e8263bfcb57446b7b2)), closes [#80](https://github.com/AbelCS/secopy/issues/80)
* **ui:** the panel's Pause doesn't flip back on an earlier update ([72a0042](https://github.com/AbelCS/secopy/commit/72a0042dae5e0c4660f54352d9e83d792f3040d4)), closes [#80](https://github.com/AbelCS/secopy/issues/80)

## [0.12.0](https://github.com/AbelCS/secopy/compare/v0.11.0...v0.12.0) (2026-09-28)


### Features

* **app:** export and import commands ([25544ea](https://github.com/AbelCS/secopy/commit/25544ea9d0f38fe4721289f0b5d5275f8399a925)), closes [#77](https://github.com/AbelCS/secopy/issues/77)
* **app:** Import… and Export… in the File menu; open .secopy from Finder ([7958928](https://github.com/AbelCS/secopy/commit/795892883da9022c7debc73d411af15bd06bf6ca)), closes [#77](https://github.com/AbelCS/secopy/issues/77)
* **app:** work out and apply an import ([2477ddd](https://github.com/AbelCS/secopy/commit/2477dddbeadf570160903299b34da47c734a12a2)), closes [#77](https://github.com/AbelCS/secopy/issues/77)
* **app:** write and read .secopy files ([aa183bf](https://github.com/AbelCS/secopy/commit/aa183bf9ce71867314c6f7306069c409d8f24ba1)), closes [#77](https://github.com/AbelCS/secopy/issues/77)
* **ui:** export settings and presets ([d875ec4](https://github.com/AbelCS/secopy/commit/d875ec41769a421369c4d4b0da42405f10b243dc)), closes [#77](https://github.com/AbelCS/secopy/issues/77)
* **ui:** the Import screen ([d115f18](https://github.com/AbelCS/secopy/commit/d115f18bc0cd2df33921ccd47a77ccb2f089829a)), closes [#77](https://github.com/AbelCS/secopy/issues/77)


### Bug Fixes

* **app:** findings from reviewing export and import ([cdf0a45](https://github.com/AbelCS/secopy/commit/cdf0a45e431d47aa8cb8a8bc773f9ddcd6311cad)), closes [#77](https://github.com/AbelCS/secopy/issues/77)
* **ui:** findings from reviewing export and import ([dcc4b4d](https://github.com/AbelCS/secopy/commit/dcc4b4dfe19b3d49698d288597eef804cf201a56)), closes [#77](https://github.com/AbelCS/secopy/issues/77)

## [0.11.0](https://github.com/AbelCS/secopy/compare/v0.10.1...v0.11.0) (2026-09-28)


### Features

* **ui:** help on buttons whose short label hides the detail ([1c740c1](https://github.com/AbelCS/secopy/commit/1c740c1d01074fa0ed42177e21d4aad2cf6bc328)), closes [#75](https://github.com/AbelCS/secopy/issues/75)
* **ui:** shorter button labels ([a6320f8](https://github.com/AbelCS/secopy/commit/a6320f80bb3ad4e029e53eff75cd1edbea9e5638)), closes [#72](https://github.com/AbelCS/secopy/issues/72)

## [0.10.1](https://github.com/AbelCS/secopy/compare/v0.10.0...v0.10.1) (2026-09-28)


### Bug Fixes

* a Verify that stopped reads as a Verify ([06aff47](https://github.com/AbelCS/secopy/commit/06aff4722ab3fdf06aea049ff52b81c929ed08ef)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** a job's ending after a panic survives the same bug ([83c5dcf](https://github.com/AbelCS/secopy/commit/83c5dcf1a09a99b0ca60fe298e55ded799fa889f)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** a queue that panicked keeps every job's real result ([b4a5552](https://github.com/AbelCS/secopy/commit/b4a55521f8372e8a81d30712e13de641069534e3)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** a scan from before a session restart stays stale ([5f730c3](https://github.com/AbelCS/secopy/commit/5f730c3a7256c8f5bb874f835ed043eee7a41c1d)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** a stopped job's queue reason ends with a full stop ([c3f2f1d](https://github.com/AbelCS/secopy/commit/c3f2f1daf0b3695fb16e0b7366bdd9e172a5d4ee)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** a Verify problem says which checksum file listed it ([04a66d6](https://github.com/AbelCS/secopy/commit/04a66d66fc372df563c310d188518f7222eba552)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** allow only the notification calls the window makes ([049ce2b](https://github.com/AbelCS/secopy/commit/049ce2b5db919dc0f5854a90fecad346826ee122)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** don't remember a full screen or maximized window's size ([c28887f](https://github.com/AbelCS/secopy/commit/c28887f11db8138ef3aa95858d6930337255e15f)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** end a job or queue whose thread panicked ([4ba13c0](https://github.com/AbelCS/secopy/commit/4ba13c079c66b52fbf325055d6f0a8e594ba55f9)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** keep working after a panic while a lock was held ([8022561](https://github.com/AbelCS/secopy/commit/80225611b2d52091a6f71d7e205d0329317d32f2)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** put profiles read from disk right like new ones ([65a3295](https://github.com/AbelCS/secopy/commit/65a3295e17b6b536e27ade88c86e9c80db22a2d1)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **app:** the profile's directory in other letter case is no change ([0831e2d](https://github.com/AbelCS/secopy/commit/0831e2d6a213a03524b1b211541fe1c2d5947748)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **core:** make a check's report read as a verify, not a copy ([db284e6](https://github.com/AbelCS/secopy/commit/db284e6d5c9bc1928df419a3c159d4bf421c71b1)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **core:** name a listed link or directory instead of "missing" ([1e07b76](https://github.com/AbelCS/secopy/commit/1e07b763366138b200a732f8e813254bc1dfa685)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **core:** name the checksum file that listed each problem file ([2bb1c57](https://github.com/AbelCS/secopy/commit/2bb1c57fb54c863d8ac78867ddf13d84abdbe702)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **core:** read from the device without following a link ([684719f](https://github.com/AbelCS/secopy/commit/684719f895bc0ddf4d8d679252e19c9740ad7ab4)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **core:** sync the directory after replacing a checksum file ([be0de7f](https://github.com/AbelCS/secopy/commit/be0de7f26c8421f440df0a1c97b4144407008ead)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* count every checksum file problem past the first 1,000 ([ab8d0ff](https://github.com/AbelCS/secopy/commit/ab8d0ffb345a85156380730352df23ea41f4f62f)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* say when a cancelled job puts the destination back ([44fc3c8](https://github.com/AbelCS/secopy/commit/44fc3c8079686229a987b643cb2a613056620652)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** ask "Discard changes?" once, however often Esc is pressed ([3824b35](https://github.com/AbelCS/secopy/commit/3824b35d78a38834d07e232f5afdb10d57de91db)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** close Save as new… and its error on another profile or source ([6782ee3](https://github.com/AbelCS/secopy/commit/6782ee3680e93ea3c9e0754899415be248dceb0d)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** name each Summary button in the queue summary for VoiceOver ([4fe7ad5](https://github.com/AbelCS/secopy/commit/4fe7ad5f8c8ad6912e3e602c01696457bd7ea169)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** name the job's kind when asking to stop or quit ([1bdad33](https://github.com/AbelCS/secopy/commit/1bdad3323986d52691ff5fa4688d194a1a7623ab)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** name the queue's list of jobs for VoiceOver ([cd6a099](https://github.com/AbelCS/secopy/commit/cd6a0998c43d67e4948aa1daff79f1448bafa974)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** quitting while files are removed says Secopy finishes first ([1bd33c9](https://github.com/AbelCS/secopy/commit/1bd33c969d29537f6845f0692ed1832d9007769a)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** send the File menu state only when it changes ([0ab4113](https://github.com/AbelCS/secopy/commit/0ab4113cd72c5bc19a67a3f3d915a2c64a3840c4)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** split file types typed together, and * means All types ([e92fd52](https://github.com/AbelCS/secopy/commit/e92fd527436e55506a5945323fd37ee8a9084d95)), closes [#69](https://github.com/AbelCS/secopy/issues/69)
* **ui:** turn off Cancel Copy in the menu while a mirror removes files ([b9230ef](https://github.com/AbelCS/secopy/commit/b9230efabd34d6cef7e64746efe48a11509f855c)), closes [#69](https://github.com/AbelCS/secopy/issues/69)


### Performance Improvements

* **core:** read a check's large files with the copy's few lanes ([a38f14e](https://github.com/AbelCS/secopy/commit/a38f14e1bac2845a931dae87c21a0a6ed8788021)), closes [#69](https://github.com/AbelCS/secopy/issues/69)

## [0.10.0](https://github.com/AbelCS/secopy/compare/v0.9.0...v0.10.0) (2026-09-28)


### Features

* **app:** check a directory, run and queue it ([a0dcb30](https://github.com/AbelCS/secopy/commit/a0dcb30633772ee163fbc799af4e04ce0c21809a)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **app:** run a check as a job ([b417217](https://github.com/AbelCS/secopy/commit/b417217e8664dbdd9e239c9ab70ba06893ffc9ec)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **cli:** --check a directory ([4ab298c](https://github.com/AbelCS/secopy/commit/4ab298cd6237dcd6450c9b24b3ffd87fd5598352)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **core:** a report for a check ([f142556](https://github.com/AbelCS/secopy/commit/f1425564371583fc48073576c4b5b56996d3a8e2)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **core:** check a directory against its checksum files ([af1cab7](https://github.com/AbelCS/secopy/commit/af1cab79bd7bbfcb22c2cb5ce1ef6acbef57cbe1)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **core:** mirrors keep a checksum file ([8d50f92](https://github.com/AbelCS/secopy/commit/8d50f92ccac0d06719d12dcbaa5ede015b825449)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **core:** plan a check of a directory's checksum files ([dc81390](https://github.com/AbelCS/secopy/commit/dc813907cff71703771382668038c6ee2742e32c)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **core:** read checksum files ([96721b6](https://github.com/AbelCS/secopy/commit/96721b69dda7841596e1eb0925db1a5f0177d060)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **ui:** the sections are tabs at the top ([cbfa566](https://github.com/AbelCS/secopy/commit/cbfa566b4329b9bfc65c2b5df11963f615852c25)), closes [#64](https://github.com/AbelCS/secopy/issues/64)
* **ui:** the Verify tab ([ca69235](https://github.com/AbelCS/secopy/commit/ca69235b65cb42c497eee34f84ec5ba73626da86)), closes [#67](https://github.com/AbelCS/secopy/issues/67)
* **ui:** underlined tabs, the Queue apart on the right ([44e38e3](https://github.com/AbelCS/secopy/commit/44e38e392ee2b7978c9274763bf60fdb9dc3005e))


### Bug Fixes

* findings from reviewing Verify ([92cfbfd](https://github.com/AbelCS/secopy/commit/92cfbfdc4f501c7136eea88d089575f8731662c6)), closes [#67](https://github.com/AbelCS/secopy/issues/67)

## [0.9.0](https://github.com/AbelCS/secopy/compare/v0.8.0...v0.9.0) (2026-09-28)


### Features

* **ui:** explain the terms that aren't clear on their own ([33764b3](https://github.com/AbelCS/secopy/commit/33764b3aadaff7a249c91e181eda9d6d2631a87a)), closes [#57](https://github.com/AbelCS/secopy/issues/57)


### Bug Fixes

* a cancelled mirror's summary doesn't contradict itself ([ed3c9b7](https://github.com/AbelCS/secopy/commit/ed3c9b7c1d93eea165cb53b2b66ccdfda57419b7)), closes [#57](https://github.com/AbelCS/secopy/issues/57)
* a file stopped by Cancel is cancelled, not failed ([3997422](https://github.com/AbelCS/secopy/commit/3997422c616c820e7fbd0780a0e4c21d9bc39b97)), closes [#57](https://github.com/AbelCS/secopy/issues/57)
* catch a source rewritten mid-copy and an unsaved destination ([fa78ec6](https://github.com/AbelCS/secopy/commit/fa78ec62332d09a5ca20c73b9294c41de47971b0)), closes [#58](https://github.com/AbelCS/secopy/issues/58)
* findings from verifying the reliability fixes ([e4d1dad](https://github.com/AbelCS/secopy/commit/e4d1dad1cad92e0d13af566767ed659e84aeb117)), closes [#58](https://github.com/AbelCS/secopy/issues/58)
* mirror loose ends ([388074a](https://github.com/AbelCS/secopy/commit/388074abdfa762119ce6ca58c431268d0ce66b7f)), closes [#57](https://github.com/AbelCS/secopy/issues/57)
* **mirror:** never lose an old version or remove the wrong file ([3188dbf](https://github.com/AbelCS/secopy/commit/3188dbfd74576f09b5aa284f96e341b82f46690d)), closes [#58](https://github.com/AbelCS/secopy/issues/58)
* never call a job complete when part of the source wasn't read ([15bb876](https://github.com/AbelCS/secopy/commit/15bb87662c5395f92c3cd482fdf94fd64b762dca)), closes [#58](https://github.com/AbelCS/secopy/issues/58)
* queue loose ends ([3aac734](https://github.com/AbelCS/secopy/commit/3aac7341491fe61e825cfa42afde0302fc8ffca3)), closes [#57](https://github.com/AbelCS/secopy/issues/57)
* review findings on the queue and mirror loose ends ([3c80e3b](https://github.com/AbelCS/secopy/commit/3c80e3b0ea67a2c684cc65fd83f05a1d67a1d0cf)), closes [#57](https://github.com/AbelCS/secopy/issues/57)
* small files are one steady row ([989e6a4](https://github.com/AbelCS/secopy/commit/989e6a4ad86c1c94d86a1d381865b3442122d163)), closes [#57](https://github.com/AbelCS/secopy/issues/57)
* **ui:** Skip never hides behind "All files copied" ([43c140e](https://github.com/AbelCS/secopy/commit/43c140eff24df2a6e67ab74d238ed2b57e2a9adc)), closes [#58](https://github.com/AbelCS/secopy/issues/58)
* undo, empty directories and the mirror preview tell the truth ([2e3b8c8](https://github.com/AbelCS/secopy/commit/2e3b8c8c0d0f4fe60c4e14c7902f3865f7a28571)), closes [#58](https://github.com/AbelCS/secopy/issues/58)

## [0.8.0](https://github.com/AbelCS/secopy/compare/v0.7.0...v0.8.0) (2026-09-28)


### Features

* **app:** mirror presets in mirrors.json ([cf4a0a8](https://github.com/AbelCS/secopy/commit/cf4a0a8ff42ff3160d35473120ed7de33d6fbde7)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **app:** mirror presets, preview and queued mirrors ([284d5ed](https://github.com/AbelCS/secopy/commit/284d5ed2969e096d3e94a7e8d06cf7cbe1932a13)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **app:** run a mirror through the job runner ([76fc701](https://github.com/AbelCS/secopy/commit/76fc701f45eedba9e0f5b62011d3e90ba2116043)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* cancel can remove the files already copied ([25d10c9](https://github.com/AbelCS/secopy/commit/25d10c937cd1e4a337ebd4ed339ccb235909ee06)), closes [#54](https://github.com/AbelCS/secopy/issues/54)
* **cli:** --mirror ([a7242ab](https://github.com/AbelCS/secopy/commit/a7242aba2fade71dc9f0389a237cdcaba47e5aa5)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **core:** archive a replaced file's old version ([7792e97](https://github.com/AbelCS/secopy/commit/7792e97f45f364d285e0c56365e01f9fb429fe10)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **core:** finish a mirror: removals and the archive ([9ad490c](https://github.com/AbelCS/secopy/commit/9ad490c19d9486bcc1f2a3fed3e815190643d425)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **core:** plan a one-way mirror ([7fb5311](https://github.com/AbelCS/secopy/commit/7fb5311bdf14dce8ed5adcd17074578c88168d64)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **ui:** preview and run a mirror ([af3eda6](https://github.com/AbelCS/secopy/commit/af3eda6c484d79b8701dddbaf7bd4b58cf3b80e9)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **ui:** the Mirror section ([adfd88b](https://github.com/AbelCS/secopy/commit/adfd88b4aa465367b2307e94263351137432bd80)), closes [#51](https://github.com/AbelCS/secopy/issues/51)


### Bug Fixes

* **core:** a mirror never removes what it couldn't read ([5d89630](https://github.com/AbelCS/secopy/commit/5d89630abf4499c969de664c9123bec9795ec654)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **ui:** a failed file's reason can be read in the file list ([836f664](https://github.com/AbelCS/secopy/commit/836f664aede025576def253186515930416c29e6)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **ui:** a new job forgets the last mirror summary ([638c4c7](https://github.com/AbelCS/secopy/commit/638c4c749dfb1aab437890bf493da930109f3c0f)), closes [#51](https://github.com/AbelCS/secopy/issues/51)
* **ui:** say how a mirror checks its files ([3958370](https://github.com/AbelCS/secopy/commit/39583706e82e31b156bb1626544d9d8c77168365)), closes [#51](https://github.com/AbelCS/secopy/issues/51)

## [0.7.0](https://github.com/AbelCS/secopy/compare/v0.6.0...v0.7.0) (2026-09-28)


### Features

* **app:** a job queue saved in queue.json ([82d315c](https://github.com/AbelCS/secopy/commit/82d315cb08e0c9681480563ac217099a82408c4e)), closes [#50](https://github.com/AbelCS/secopy/issues/50)
* **app:** a queued copy is rebuilt and checked at its turn ([9e1e280](https://github.com/AbelCS/secopy/commit/9e1e28018477dd47d0bb20c60e492ad60ac47a6e)), closes [#50](https://github.com/AbelCS/secopy/issues/50)
* **app:** queue commands and a View menu ([88ce1cc](https://github.com/AbelCS/secopy/commit/88ce1cc40d0aa2b4991a5a4de95e932b2dd2b7cc)), closes [#50](https://github.com/AbelCS/secopy/issues/50)
* **app:** run the queue one job after another ([09eb0d1](https://github.com/AbelCS/secopy/commit/09eb0d1e64690171e5d9a2fee6b7b6c125fe29f9)), closes [#50](https://github.com/AbelCS/secopy/issues/50)
* **ui:** a sidebar with Copy and Queue ([0783d9f](https://github.com/AbelCS/secopy/commit/0783d9fe6085c5ad8ba1cc9b78ddfcf8dab12281)), closes [#50](https://github.com/AbelCS/secopy/issues/50)
* **ui:** Add to queue on New copy ([e00c2e4](https://github.com/AbelCS/secopy/commit/e00c2e4ee8189af48641b048e494d06de4d069d4)), closes [#50](https://github.com/AbelCS/secopy/issues/50)
* **ui:** run the queue, with a queue summary ([0a2d2b4](https://github.com/AbelCS/secopy/commit/0a2d2b4981e6b49377bbebd15666a5b7d2147d91)), closes [#50](https://github.com/AbelCS/secopy/issues/50)
* **ui:** the Queue screen ([4acc652](https://github.com/AbelCS/secopy/commit/4acc65280afa79470a5d315b210c6f9586aac517)), closes [#50](https://github.com/AbelCS/secopy/issues/50)


### Bug Fixes

* **app:** review fixes for the queue ([b445ca8](https://github.com/AbelCS/secopy/commit/b445ca8a1d1b8a44a3afca3d7010683c5f8c0a96)), closes [#50](https://github.com/AbelCS/secopy/issues/50)

## [0.6.0](https://github.com/AbelCS/secopy/compare/v0.5.1...v0.6.0) (2026-09-28)


### Features

* **app:** profiles save the source; Start says Start copy ([fa4376c](https://github.com/AbelCS/secopy/commit/fa4376cc9f0ec62bb078205df29343a8f3eb015e)), closes [#44](https://github.com/AbelCS/secopy/issues/44) [#45](https://github.com/AbelCS/secopy/issues/45) [#46](https://github.com/AbelCS/secopy/issues/46)
* **ui:** clearer New copy labels and status ([3cb9d42](https://github.com/AbelCS/secopy/commit/3cb9d42ed2eae9b1253147d873addb756c66f079)), closes [#42](https://github.com/AbelCS/secopy/issues/42)
* **ui:** leave ejecting to macOS ([eea4a78](https://github.com/AbelCS/secopy/commit/eea4a783e37ed2b280445ed63cb4f5d18e609800)), closes [#39](https://github.com/AbelCS/secopy/issues/39)

## [0.5.1](https://github.com/AbelCS/secopy/compare/v0.5.0...v0.5.1) (2026-09-28)


### Bug Fixes

* **app:** sign the whole app bundle so macOS doesn't call it damaged ([7526d51](https://github.com/AbelCS/secopy/commit/7526d51c6046642911493c4701bd95c41223441a)), closes [#36](https://github.com/AbelCS/secopy/issues/36)

## [0.5.0](https://github.com/AbelCS/secopy/compare/v0.4.0...v0.5.0) (2026-09-28)


### Features

* **ui:** clearer progress and file lists ([5e0e77c](https://github.com/AbelCS/secopy/commit/5e0e77c3d781cd20539e5876c46080510ea43b4c)), closes [#33](https://github.com/AbelCS/secopy/issues/33)
* **ui:** time left is just the time ([3f8e009](https://github.com/AbelCS/secopy/commit/3f8e0096f979e8e4b751a5dfb4d02ec97329dc64)), closes [#33](https://github.com/AbelCS/secopy/issues/33)


### Bug Fixes

* **core:** write the checksum file on SMB shares ([c825790](https://github.com/AbelCS/secopy/commit/c8257904ed147024482bffac67881fcbecbf4319)), closes [#32](https://github.com/AbelCS/secopy/issues/32)

## [0.4.0](https://github.com/AbelCS/secopy/compare/v0.3.0...v0.4.0) (2026-09-27)


### Features

* **app:** a File menu with shortcuts; Space pauses, Esc goes back ([5779d7a](https://github.com/AbelCS/secopy/commit/5779d7ab5bf9b042b8b85a368fbda8241935a617)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **app:** eject the card from the summary ([bcf7896](https://github.com/AbelCS/secopy/commit/bcf78965859a03b56748dc91e20e0e02b90688b5)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **app:** notify when a copy finishes in the background ([9017e6c](https://github.com/AbelCS/secopy/commit/9017e6cdd81f7218e783fe335addd01d0a36266d)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **ui:** accessibility pass: contrast, focus, announcements, axe checks ([553f7a2](https://github.com/AbelCS/secopy/commit/553f7a24de7a2699957149571ddfcaa6acde2a9a)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **ui:** Settings are saved with Save, dropped with Cancel ([d030f3d](https://github.com/AbelCS/secopy/commit/d030f3d347b9d4ee7e2f477c766a4956bc3ce4ec)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **ui:** Source shows the chosen source; no drives row ([f6afffa](https://github.com/AbelCS/secopy/commit/f6afffaa19c7952d5848ff93441840cc91153445)), closes [#30](https://github.com/AbelCS/secopy/issues/30)


### Bug Fixes

* **app:** list drives without waiting on slow file systems ([b3c3b7b](https://github.com/AbelCS/secopy/commit/b3c3b7b329d5496c9e739176563e49a9e35738ec)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **app:** review fixes for Eject, Retry and Space ([2ed876f](https://github.com/AbelCS/secopy/commit/2ed876fdc9646dc0fe02ab16431f50d198695bed)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **core:** end a job without waiting for the progress interval ([58de8f8](https://github.com/AbelCS/secopy/commit/58de8f87f4516d58a9f6dc1df37591f9ee27e732)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **core:** keep the Mac awake without a helper process ([d850bbe](https://github.com/AbelCS/secopy/commit/d850bbedcded0684c685b828368c4750e4490030)), closes [#20](https://github.com/AbelCS/secopy/issues/20)
* **ui:** a directory dropped on TO becomes the destination ([f6fc526](https://github.com/AbelCS/secopy/commit/f6fc526f00fe797ece3a7c734f32e0c423686a24)), closes [#29](https://github.com/AbelCS/secopy/issues/29)
* **ui:** Back is a button in the action bar, like every other action ([a71f6f0](https://github.com/AbelCS/secopy/commit/a71f6f0ee5f1ffcd3e20adc06f2b044204aa6976)), closes [#20](https://github.com/AbelCS/secopy/issues/20)

## [0.3.0](https://github.com/AbelCS/secopy/compare/v0.2.0...v0.3.0) (2026-09-27)


### Features

* **app:** apply source profiles to what was picked ([16e85af](https://github.com/AbelCS/secopy/commit/16e85afcf0a381060402b0cd082222717a8d1f82)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **app:** let a job skip the checksum file or save its report next to it ([3b1ae30](https://github.com/AbelCS/secopy/commit/3b1ae30e7e876a7050a5d769c01d5a3d4c6511c3)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **app:** list the drives under /Volumes ([69fb1cb](https://github.com/AbelCS/secopy/commit/69fb1cbf56327e29ee4832aa0dc2cbedc7bf640d)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **app:** profile, settings and start-up commands, remembered window and mode ([c1d90bb](https://github.com/AbelCS/secopy/commit/c1d90bbe01aeeb58818e23f4c9e5ab08dd59456f)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **app:** save settings, profiles and remembered state ([805acd4](https://github.com/AbelCS/secopy/commit/805acd4862112db04725c4ab39152a6945eb8b8c)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **app:** use the com.latecommits.secopy identifier, moving 0.2.0 reports ([d4733de](https://github.com/AbelCS/secopy/commit/d4733deb496255b481013ac50eed65b720fa0e7d)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **core:** copy hidden files, skip only system files ([764d047](https://github.com/AbelCS/secopy/commit/764d04726b98c2184632d4bc3fd2e90068bfc65f)), closes [#25](https://github.com/AbelCS/secopy/issues/25)
* **ui:** a Profiles screen of its own, and Settings with only the settings ([446b853](https://github.com/AbelCS/secopy/commit/446b853f213bd82b45b37303e20d5c30d19e520d)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **ui:** clearer main window, Settings and Profiles ([fa107a4](https://github.com/AbelCS/secopy/commit/fa107a4d2a849fd07e9ab9ca0dd7bd4b0722573a)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **ui:** design tokens and the component library ([1408c38](https://github.com/AbelCS/secopy/commit/1408c38bc52f377f0d4d40a0129ca8dc13d97907)), closes [#22](https://github.com/AbelCS/secopy/issues/22)
* **ui:** drives, source profiles and recent destinations in the main window ([ddcbb88](https://github.com/AbelCS/secopy/commit/ddcbb8876e8456096aad10e287151245bc9d4a04)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **ui:** every screen on one layout: header, sections, action bar ([7fa0105](https://github.com/AbelCS/secopy/commit/7fa01051432aa57fc2c6a6f110289eba6b019923)), closes [#22](https://github.com/AbelCS/secopy/issues/22)
* **ui:** labelled rows inside sections ([78f22d9](https://github.com/AbelCS/secopy/commit/78f22d9cdb48cb6f894e405d4f90ac535c792207)), closes [#22](https://github.com/AbelCS/secopy/issues/22)
* **ui:** the Settings screen, with source profiles ([3f1c6f9](https://github.com/AbelCS/secopy/commit/3f1c6f9e4387f6dfdde9e81aa93e73d36f31ef52)), closes [#16](https://github.com/AbelCS/secopy/issues/16)


### Bug Fixes

* **app:** let saves at the same time neither fail nor undo each other ([d74a900](https://github.com/AbelCS/secopy/commit/d74a9001bc94b3f6e27f8a907611448c8ad5e2a6)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **app:** list only real drives in FROM ([9f7f8e1](https://github.com/AbelCS/secopy/commit/9f7f8e17b5419008776d9b8f8dc8059e13125496)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* **app:** wait for a pending scan before saving a profile or keeping file types ([49229a1](https://github.com/AbelCS/secopy/commit/49229a16855693c5ea2b48f2ce6e415f6ce37659)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* say directory, not folder, everywhere the user reads it ([6e5f30c](https://github.com/AbelCS/secopy/commit/6e5f30c8e7d3e13cdb8054ac84519ffa49ad6c8a)), closes [#16](https://github.com/AbelCS/secopy/issues/16)
* say nothing about a checksum file when it is turned off ([4053e77](https://github.com/AbelCS/secopy/commit/4053e77126d964780667860dc992670e505153f9)), closes [#16](https://github.com/AbelCS/secopy/issues/16)

## [0.2.0](https://github.com/AbelCS/secopy/compare/v0.1.0...v0.2.0) (2026-09-27)


### Features

* **app:** add the tauri app and svelte ui scaffold ([4cc5977](https://github.com/AbelCS/secopy/commit/4cc5977601fe2e1dafa5a07e3671055348df07d7)), closes [#15](https://github.com/AbelCS/secopy/issues/15)
* **app:** expose commands and generate typescript bindings ([3642685](https://github.com/AbelCS/secopy/commit/36426851b2d03aa412423939b898ed5e913119c3)), closes [#15](https://github.com/AbelCS/secopy/issues/15)
* **app:** keep the main window's session state ([12b75bc](https://github.com/AbelCS/secopy/commit/12b75bc813f0869f375ac9b55e54eb8f83637566)), closes [#15](https://github.com/AbelCS/secopy/issues/15)
* **app:** run copy jobs from the app ([7908c36](https://github.com/AbelCS/secopy/commit/7908c364eceb852dea8d8a5c0ca538185aead5ed)), closes [#15](https://github.com/AbelCS/secopy/issues/15)
* **ui:** add formatting, speed meters and the app api ([92ca1b6](https://github.com/AbelCS/secopy/commit/92ca1b6c091ef8c3851f22515237f2e4627d250a)), closes [#15](https://github.com/AbelCS/secopy/issues/15)
* **ui:** add the main window ([43d3e2b](https://github.com/AbelCS/secopy/commit/43d3e2b11495a7ebdf9989d22e79079076957036)), closes [#15](https://github.com/AbelCS/secopy/issues/15)
* **ui:** add the progress view ([a1f6638](https://github.com/AbelCS/secopy/commit/a1f66382bb7f70741a25159b6a0adb3bef92fd5f)), closes [#15](https://github.com/AbelCS/secopy/issues/15)
* **ui:** add the summary and connect the screens ([d6e3506](https://github.com/AbelCS/secopy/commit/d6e35067cd295faa4a18036912b25972a7bef721)), closes [#15](https://github.com/AbelCS/secopy/issues/15)
* **ui:** list every file on the summary ([b7361bf](https://github.com/AbelCS/secopy/commit/b7361bfcd1ea7580a8994f87e1cdc58a5760b209))
* **ui:** one Choose… for a folder or files, and an Include folder checkbox ([600fdad](https://github.com/AbelCS/secopy/commit/600fdad1feb1b55157417783c04e0fac67d9708d))


### Bug Fixes

* **app:** keep a stopped job's bars honest, say why a report wasn't saved ([16640f3](https://github.com/AbelCS/secopy/commit/16640f3e3de3bb746a85dfba8c7c50a222968331))
* **app:** let the window close, and ask before quitting during a copy ([7c91fbb](https://github.com/AbelCS/secopy/commit/7c91fbbb6889206aeaf374fe491203cbef876fd0))
* **ui:** ask for a short page once per update, and scroll up for Failed only ([9baf750](https://github.com/AbelCS/secopy/commit/9baf7500750e181102a87c4b6011b7816e2a6df5))
* **ui:** show scans in progress, and keep a retry in the card's folder ([7b46fa1](https://github.com/AbelCS/secopy/commit/7b46fa1752d22796d6878b9579813801e44d1b23))

## 0.1.0 (2026-09-27)


### Features

* **cli:** add conflict choice, pre-flight output and report files ([4427026](https://github.com/AbelCS/secopy/commit/442702694fa28863eecbfdca70d5fbacaebf6e61))
* **cli:** add secopy-cli for development and benchmarks ([77858b8](https://github.com/AbelCS/secopy/commit/77858b8b50b065e472284689ebbe77ff70aa1d2b))
* **core:** add cargo workspace and xxhash64 helpers ([167d526](https://github.com/AbelCS/secopy/commit/167d5266eb2c58cb67cc0b7ccdee6d4e3953ee01))
* **core:** add pre-flight checks ([50c7752](https://github.com/AbelCS/secopy/commit/50c7752b8beae73c1f64efffe6e441910df7d3f2))
* **core:** add source model, extension filter and hidden detection ([82c1f5c](https://github.com/AbelCS/secopy/commit/82c1f5c073e50ea3762479d499fb297376f568bf))
* **core:** check names against the destination file system ([38a0313](https://github.com/AbelCS/secopy/commit/38a03137316dbe8f769ba81693fd822f2eea7a06))
* **core:** commit copies by replacing or under a numbered name ([db25043](https://github.com/AbelCS/secopy/commit/db250436612681783610d54c242eb35ea17e1192))
* **core:** copy files to partial files with inline xxhash64 ([ccc9caf](https://github.com/AbelCS/secopy/commit/ccc9caf200466b27114f0d5347c4550de81100b8))
* **core:** keep file and folder metadata on copies ([a5e4a3f](https://github.com/AbelCS/secopy/commit/a5e4a3f1da56792e27dc81dc056241542737fd7d))
* **core:** keep the system awake during a job ([9783ea4](https://github.com/AbelCS/secopy/commit/9783ea40a4d91cef6e01a1fff8bcce244c376ccc))
* **core:** pause and resume a running job ([8483ceb](https://github.com/AbelCS/secopy/commit/8483ceb89294411e6477f631714638e47a58473e))
* **core:** read destination file system facts ([f8dc967](https://github.com/AbelCS/secopy/commit/f8dc96735badb85ff9c1544fdada1722f2f13e27))
* **core:** record mtimes and resolve source paths without canonicalize ([023193c](https://github.com/AbelCS/secopy/commit/023193cae37355c070d9ed0962b0b9366d37a471))
* **core:** resolve a plan with one action per file ([1029682](https://github.com/AbelCS/secopy/commit/1029682f824892972879186e96101984e8d1e361))
* **core:** run copy jobs with verify lanes, retries and progress events ([94a2082](https://github.com/AbelCS/secopy/commit/94a20826c001794aa0da7c6bd55a7c6e653289fb))
* **core:** run jobs from a resolved plan ([5d914ea](https://github.com/AbelCS/secopy/commit/5d914eaf47b38b490a384f34ff0fc347daf7aba3))
* **core:** scan sources and select files by extension ([5ed3cec](https://github.com/AbelCS/secopy/commit/5ed3cecfeeb3b1b31b35e012ae2ca7f903c12e9b))
* **core:** stop the job when the source or destination goes away ([3e88fd6](https://github.com/AbelCS/secopy/commit/3e88fd66b4a870e94701be5516ecb6f5057912e2))
* **core:** verify copies from the device with cache bypass ([2d4d2bd](https://github.com/AbelCS/secopy/commit/2d4d2bd587963100d667634cb9da191c31ddf3f0))
* **core:** write the job report as text and json ([c87ac82](https://github.com/AbelCS/secopy/commit/c87ac8244b20d499a89b235223c3dcaf14e40c90))
* **core:** write xxhsum-compatible checksum files ([3cd8fec](https://github.com/AbelCS/secopy/commit/3cd8feccc44d6349ffad8ffd3932dc4c2f285a7a))


### Bug Fixes

* **core:** detect a lost source when it was given as a relative path ([ca83504](https://github.com/AbelCS/secopy/commit/ca8350493b708c726feae3fac14c82035ba0f0dd))
* **core:** keep verifying copied files when the source disappears ([75e1263](https://github.com/AbelCS/secopy/commit/75e1263bdf961cbe088da24375098dcec106fcd1))
* **core:** lock partial files and replace ones left by interrupted jobs ([bebdebe](https://github.com/AbelCS/secopy/commit/bebdebe5593ce161ba726c911fc92d86c63e9e51))
* **core:** never overwrite another writer's file, end jobs on panic, bound the verify queue ([9c44046](https://github.com/AbelCS/secopy/commit/9c44046d2eb447f7f7585afc17ba44bc9bfc8100))
* **core:** only commit or delete a partial file that is still ours ([248241e](https://github.com/AbelCS/secopy/commit/248241eec8a74090b239d8b0211788c1e127ce30))
* **core:** remove leftover partial files next to skipped files ([24f6ff8](https://github.com/AbelCS/secopy/commit/24f6ff8481b806c63d53a1bfd65f71ea3f181c88))
* **core:** support long paths in raw windows calls ([a6de074](https://github.com/AbelCS/secopy/commit/a6de0743261cd51c62fe0d4f4a6dc23ab49e5b42))
* **core:** treat future-dated leftover partial files as stale ([71358ef](https://github.com/AbelCS/secopy/commit/71358ef8ede3bd20e2ade161247e0859a53cefeb))

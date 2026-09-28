# Changelog

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

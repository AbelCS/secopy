# Changelog

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

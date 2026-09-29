# Changelog

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

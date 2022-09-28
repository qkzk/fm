# FM : Dired like File Manager

## DONE

- [x] filetype
  - [x] identifier filetype cf TODO
  - [x] colorier selon filetype cf TODO
- [x] scroll
<!-- TODO: bug quand on a trop de fichiers, on peut scroll jusqu'en bas -->
- [x] load from anywhere
- [x] args : dirpath & show hidden (-a)
- [x] toggle hidden
- [x] spawn a shell, open with xdg-open
- [x] manipuler :

  [fuzzy finder in tuiki](https://github.com/lotabout/skim/blob/master/src/input.rs)

  - [x] flagged
  - [x] rename
  - [x] supprimer
  - [x] insérer fichier / dossier
  - [x] chmod parsed from octal
  - [x] cut copy paste
  - [x] spawn shell in dir
  - [x] open file with xdg-open
  - [x] open file with custom command

- [x] usage
- [x] help menu
- [x] custom config file
- [x] custom keybindings
- [x] GOTO mode
- [x] batch chmod
- [x] mouse support
  - [x] left move index to this file
  - [x] right on dir enter
  - [x] right on file open
  - [x] up / down
- [x] links are followed
- [x] resize (i guess it's an event like in curse) seems to work already
- [x] dirsymbol for sockets and whatever
- [x] refactor FileInfo, use an enum
- [x] '\*' flag all
- [x] v reverse flags
- [x] allow '~' in GOTO mode
- [x] regex
  - [x] search
  - [x] mark multiple files
- [x] jump pour next flagged file

## TODO

- [ ] remote control
  - [ ] listen to stdin (rcv etc.)
  - [ ] nvim plugin
  - https://github.com/KillTheMule/nvim-rs/blob/master/examples/basic.rs
  - https://neovim.io/doc/user/api.html
- [ ] installed config file, user config file
- [ ] completion
  - [x] in goto mode,
  - [ ] exec mode,
  - [ ] searchmode (???)
- [ ] confirmation for cut/paste, copy/paste, delete

## BUGS

- [ ] when opening a file with rifle opener into nvim and closing, the terminal hangs
- [ ] can navigate outside file list
- [ ] strange behavior after leaving a mode, wrong files are flagged

## Configuration sources & ideas


1. struct without option and default hardcoded values
2. look for config file in $HOME/.config/fm
4. don't update, replace


## completion

workflow in [#10](https://github.com/qkzk/fm/issues/10)

## Sources

### Configuration

- [rust cli book](https://rust-cli.github.io/book/in-depth/config-files.html)
- [crate confy](https://docs.rs/confy/latest/confy/fn.get_configuration_file_path.html)
- [crate configure](https://docs.rs/configure/0.1.1/configure/)

### CLI

- [CLI crates](https://lib.rs/command-line-interface)

### filepicker in vim

- [fff](https://github.com/dylanaraps/fff)

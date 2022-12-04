# FM : Dired like File Manager

## DONE

- [x] filetype
  - [x] identifier filetype cf TODO
  - [x] colorier selon filetype cf TODO
- [x] scroll
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
- [x] user config file
- [x] completion
  - workflow in [#10](https://github.com/qkzk/fm/issues/10)
  - [x] in goto mode,
  - [x] exec mode,
  - [x] searchmode
- [x] confirmation for cut/paste, copy/paste, delete
- [x] bugfix: strange behavior after leaving a mode, wrong files are flagged - not in the right index or something
- [x] bugfix: can navigate outside file list
- [x] sorting : filename, size, date, type
- [x] refactor key(char) -> action
  - [x] enum for actions
  - [x] hmap for keybindings
  - [x] key -> action -> status.update(action)
  - [x] association with match and clear code
  - [x] dissociate keybinding from status update
- [x] fix: cursor is hidden before leaving the application
- [x] create a symlink to flagged files
- [x] preview a file with P
  - [x] preview navigation, integrate into file_window,
  - [x] preview content in head not stack
  - [x] syntax highlighting with [syntect](https://github.com/trishume/syntect)
  - [x] binary preview
- [x] history of visited files: use a Vec as a stack [collections](https://doc.rust-lang.org/std/collections/index.html)
- [x] shortcuts
- [x] multiple tabs: TAB to switch, DEL to drop, INS to create. Flagged files are shared between tabs.
- [x] rename file_window to content_window ?
- [x] improve the top row
- [x] confirmation display shows a list of edited files
- [x] integrate fzf or another fuzzy finder
- [x] custom a file opener
- [x] bulkrename @ ranger
- [x] scrollable help
- [x] user defined marks ; saved and read from a file.
- [x] refactor: main should return result, have everything raise errors
- [x] stable colors per extension with caching
- [x] BUGFIX creating an already existing dir / file crashes
- [x] display link destination
- [x] copy filename/filepath to clipboard with ctrl+c & ctrl+p
- [x] filters by ext / name / only dirs / all (aka no filter)
- [x] FIX: broken links aren't shown
- [x] COPY improvment
  - [x] async/threaded copy -- move & delete should be quick enough
  - [x] progress bar for copy
  - [x] move/copy progress displayed, nothing else
  - [x] display copy/move with style, refresh when done (reset file position)
- [x] FIX: opener crash, right on file crash when in nvim toggleterm
- [x] FIX: marks saved without newlines
- [x] drag & drop: exec and find dragon-drop
- [x] optional numbers in preview
- [x] logging with rotating log files.
- [x] git integration in first line of normal mode.
- [x] display space used (only the current folder, no recursive total)
- [x] display the free space
- [x] reduce release binary size a bit (12M -> 6M)
- [x] FIX: disk space is always showing the same disk
- [x] Colored first line
- [x] Resize immediatly
- [x] display should be "terminal manager" and it shouldn't handle anything else (git, available space etc.)
- [x] preview EXIF for image files
- [x] media info for video file / audio file
- [x] fix wrong position of cursor
- [x] improve tabs interface
  - [x] tab bar
  - [x] digit move to respective tab
  - [x] <TAB> creates a new tab if only one
  - [x] <BACKTAB> moves to previous tab
  - [x] hardcoded limit to 10 tabs
- [x] print selected path on quit
- [x] Alt+d call dragon-drop on selected file
- [x] cd on quit:

  fm prints its current directory when exiting

  1. Install a link to `fm` in your path or copy the binary

  2. Add this to .zshrc :

     ```bash
     function f() {
       dest=$(fm $@)
       if [[ ! -z $dest ]]
       then
         cd $dest
       fi
     }
     ```

- [x] Refactor preview using a common trait & macros
- [x] fix previewing non normal file hangs by preventing previewing...
- [x] send a notification when files are copied or moved
- [x] regex matcher (w) updates as you type
- [x] help displays current keybindings
- [x] dual pane. Only two tabs
- [x] allow multiple keybindings for same action

## TODO

- [ ] remote control
  - [x] filepicker
        requires the nvim-remote rust crate installed
  - [ ] listen to stdin (rcv etc.)
    - [ ] follow change directory
    - [ ] when called from a file buffer in nvim, open with this file selected
  - [ ] nvim plugin - set a serverstart with a listenaddress, send it to fm
  - https://github.com/KillTheMule/nvim-rs/blob/master/examples/basic.rs
  - https://neovim.io/doc/user/api.html
  - [ ] $NVIM_LISTEN_ADDRESS isn't always set on nbim startup ; can be set from nvim before running... then sent to fm with some args
  - [ ] args read correctly, use NVIM_LISTEN_ADDRESS if args is sent
- [ ] display / event separation. use async and message passing between coroutines
- [ ] preview images @ranger [ueberzug-rs](https://github.com/Adit-Chauhan/Ueberzug-rs) @[termimage](https://rawcdn.githack.com/nabijaczleweli/termimage/doc/termimage/index.html)
- [ ] compression

  - [ ] compress
  - [x] decompress selected file with ctrl+x
  - [x] preview

- [ ] plugins
  - which language ?
  - what for ?
- [ ] configurable output (owner, group etc.)

## Won't do

### auto stuff

All of this stuff can be done easily through a shell command or automatically. I'm not sure I wan't to bloat fm with it.

- [ ] auto mount usb keys ??? [rusb](https://github.com/a1ien/rusb) -- just use udiskie (started automatically) and udiskie-umount /mount/point
      just use udiskie
- [ ] mtp... but fast [libmtp.rs](https://docs.rs/libmtp-rs/0.7.7/libmtp_rs/)
- [ ] connexion to remote servers [removefs](https://crates.io/crates/remotefs) [termscp](https://crates.io/crates/termscp)

  - ssh
  - sftp
  - ftp
  - google drive

  or just use sshfs...

## BUGS

- [ ] when opening a file with rifle opener into nvim and closing, the terminal hangs

## Sources

### CLI

- [CLI crates](https://lib.rs/command-line-interface)

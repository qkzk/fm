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
- [x] single pane if width is low
- [x] disks:
  - [x] simplify disk space read,
  - [x] hold a sys reference in status
  - [x] add shortcut to mount points
- [x] dissociate action from status / tab
- [x] opener fallback to xdg-open, capture stderr & stdout
- [x] toggle between simple & complete output

- [x] make every key configurable
  - [x] syntax able to parse any combination of key
  - [x] config parser -> `Keybindings { binds: HashMap<Key, ActionMap>}`
  - [x] help display
  - [x] link any event to actionmap
  - [x] display every event in help
- [x] FIX. displaying marks also shows a char from previous mode at end of line. Don't add "\n"...
- [x] FIX: open, visite, go back then display history -> crash.
- [x] FIX wrong pattern in mark file caused crash. Rewrite file if wrong pattern found.
- [x] Compressed files:
  - [x] Opening a supported compressed file decompress it.
  - [x] Preview a compressed file displays its content
- [x] preview images
  - [x] display an image as a pixeled thumbnail. IDK how to integrate ueberzug-rs / ueberzug into tuikit so it's an acceptable solution. The result is ugly.
  - [x] default preview exif
  - [x] char('T') for thumbnail
- [x] togglable dual panes... with default mode for low size
- [x] FIX: non ascii typed symbols crash the application.
      Don't use non ascii chars atm. It's hard to navigate in a string of non ascii chars and
      would require another crate.
- [x] non ascii char support. ie graphemes. Simply use a vec of chars and collect it when needed.
- [x] Fix: / (slash) in newfile, newdir crashes the app with strange errors. Use sanitize_filename
- [x] goto relative path. Look for directory in current path
- [x] keeps searching for same result with f.

  When a search is made (/),

  - if the user completes (TAB) and Enter, only this file can be found.
  - if the user doesn't complete but Enter immediatly, we can find any file containing this name.

    The user can search from next element with f.

- [x] publish 0.1.0 on [crates.io](https://crates.io/crates/fm-tui)

  - [x] documentation
  - [x] move strings to separate file
  - [x] build script
  - [x] readme for user not developpers, move readme to dev.md
  - [x] fix dependencies (skim-qkzk instead of a git version, no wildcards, tuikit 0.4.5 for skim)
  - [x] publish on cargo

### Version 0.1.1

- [x] NeedConfirmation should take a parameter, avoiding an attribute in tab/status.

- [x] Mode Refactor.

  use child enum to simplify Modes

  - [x] group every mode requiring input (2 variants with subvariants: with or without completion)
  - [x] combine help & preview
  - [x] better first line for mode filter
  - [x] FIXED: help has wrong first line

### Version 0.1.2 : refactor preview

- [x] refactor Preview::new() refactored and simplified

### Version 0.1.3 : cd ..

- [x] parse .. in goto

### Version 0.1.4 : animation, refactoring

- [x] Gif animation in readme
- [x] syntactic sugar to create custom errors
- [x] filter improvment: better display, use &str when possible
- [x] sort refactor. Use a crate. Use 2 methods for ascending/descending. Separate
      char parsing from sort itself
- [x] Fix: in long dirs, we can scroll past the last displayed file
- [x] Fix: multiple mount points display in shortcuts.
- [x] trait for struct or thing with index

  - [x] trait<T> { collection: Vec<T>, index: usize}: next, prev etc.
  - [x] harmonize code for multiple instances
  - [x] macro to impl auto
  - [x] struct for flagged. Use a vector instead of hashset... :(
  - [x] regroup shortcut, history, jump, visited
  - [x] improve flagged complexity using binary search since the content is maintened sorted.

### Version 0.1.5

- [x] Fix scrolling in normal & preview modes
- [x] refactor search next
- [x] refactor display of selectable content

### Version 0.1.6

- [x] Prevent entering confirmed actions (copy, delete, move) if no file is flagged
- [x] Improve saved marks display by using a BTreeMap, allowing sorting by char.
- [x] Prevent entering jump marks mode if there's no mark to jump to

### Version 0.1.7

- [x] Trash respecting [trashspec](https://specifications.freedesktop.org/trash-spec/trashspec-latest.html)

  - [x] trahinfo
  - [x] recreate parents if needed
  - [x] allow multiple files with same name to be trashed
  - [x] allow strange bytes in path
  - [x] compatibiliy with other programs like trash-cli

- [x] preview directory with tree view [termtree](https://crates.io/crates/termtree)
- [x] refactoring: remove many PathBuf, String, to_owned, clone... and other heap allocation.
      I tried to use as many reference as I could.
- [x] userscache. Cache users & group at launch. Refresh them when refreshing views.
- [x] show . & .. in normal display

### Version 0.1.8

- [x] improve fuzzy finding by moving to the selected file
- [x] use latest version of skim-qkzk

### Version 0.1.9 : tree view

New view: Tree ! Toggle with 't', fold with 'z'. Navigate normally.

- [x] display a tree view with T
- [x] navigate in the tree
- [x] enter a directory from the tree
- [x] enter a file:
  - [x] enter parent
  - [x] select the file
- [x] scrolling
  - [x] last file can't be at top
- [x] enable most modes
  - [x] copy, cut, delete, trash, search
- [x] enable most actions
  - [x] END move to last leaf
  - [x] toggle flag and display flagged files
  - [x] copy filename, filepath
  - [x] toggle hidden
  - [x] drag n drop
  - [x] symlink
- [x] disabled:
  - [x] regex match: would only in current root path
  - [x] sort: would require recursive sort of every directory
- [x] replace preview::directory by mode::tree
- [x] filter
  - [x] force display full in tree mode
- [x] search simple: first result
- [x] fold
  - [x] fold a single directory
  - [x] display a triangle to display folded status
  - [x] unfold all, fold all

### Version 0.1.10 : keep track previous mode

- [x] return to previous mode when executing (ie. pressing Enter) or leaving a mode which mutates the content (like sort).
- [x] tree: sort.

### Version 0.1.11 :

- [x] window for mode displaying info (completion, marks, shortcut, jump)
- [x] clicks on right pane should select it first
- [x] doucle click left = right click : open file
- [x] wheel select pane
- [x] enable mouse in tree mode
  - [x] wheel -> next/prev sibling (somewhat okay)
  - [x] left/right
- [x] FIX: quit from preview is weird

### Version 0.1.12 :

- [x] FIX: filter aren't applied at all

### Version 0.1.13 :

- [x] luks encryption

  Open & mount, umount & close luks encrypted partitions.

  - [x] menu with Shift+e
  - [x] mount with m
  - [x] unmount with u

  ask for a sudo password and luks passphrase.

  _should_ work with other kind of encryption. Can't test it since I don't have another disk for that purpose...

### Version 0.1.14 :

- [x] improve tree navigation: up & down can jump to the node immediatly below in the view

### Version 0.1.15 :

- [x] Alt+c opens the config file
- [x] Add a shortcut to the config folder
- [x] use g to go to the mounted encrypted drive

### Version 0.1.16 : fix completion & filter in tree

- [x] FIX: can't parse uid gid if they only exists on a remote machine. See https://serverfault.com/questions/514118/mapping-uid-and-gid-of-local-user-to-the-mounted-nfs-share
      for a fix.
- [x] FIX: truncate file size in preview mode.
- [x] FIX: in tree mode search is backward
- [x] FIX: when searching from tree mode, it only completes with level 1 elements, not nested ones.
- [x] FIX: when exiting search in tree mode, second line isn't updated
- [x] FIX: when filtering in tree mode, only the level 1 matching elements are displayed
      Decided to keep directories when filtering in tree mode. Those are excluded when filtering in normal mode.
- [x] Tree: move 10 rows at a time

### Version 0.1.17 : git root, navigable marks, compression/decompression, command mode, lazygit

- [x] git root: cd to git root
- [x] tree: use the length of the screen to avoid parsing non displayed lines
- [x] navigate in marks: pick a mark and jump to it with enter
- [x] compress flagged files: pick a compression algorithm from a list.
- [x] decompress any archive we can create
- [x] command mode with ":" and a command. (ie `:ClearFlags`). Command are completed.
      some commands does nothing :(
- [x] lazygit integration: open a terminal with lazygit in current path. Obviously [lazygit](https://github.com/jesseduffield/lazygit) should be installed.

### Version 0.1.18 : ???

- [x] preview completion in another color
- [x] use notify-send instead of a crate to lower binary size
- [x] use ueberzug instead of ugly thumbnail. Requires ueberzug to be installed.
- [x] use second pane for preview, update auto
  - [x] FIX: second pane preview has wrong size
- [x] improve directory preview by limiting depth
- [x] preview videos with a thumbnail (created with ffmpeg)
- [x] use media info for audio
- [x] remove some attributes from FileInfo, lowering its size.
- [x] FIX: sometimes pdf_extract prints to stdout. A fix is proposed in a PR, atm we'll use Gag to redirect stdout.
- [x] FIX: prevent invalid zips from crashing the app while previewing
- [x] FIX: preview of big highlighted source file is slow. Fix it by preventing those preview for files larger than 32kiB.
- [x] display the number of flagged files
- [x] improve preview first line
- [x] FIX: previewing a file without read permission crashes. Use Empty preview as default.
- [x] regroup display settings keybindings under alt+char
- [x] restrain second pane to 120+ chars wide terminals

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

- [ ] vim keys, harmonize keybinds with ranger
- [ ] zoxide support
- [ ] make navigable content scrollable
- [ ] temporary marks
- [ ] context switch
- [ ] read events from stdin ? can't be done from tuikit. Would require another thread ?
- [ ] pushbullet ?

- [ ] Version 0.1.19

  - [x] skim: use preview from bat.
  - [x] skim: preview with cat if bat isn't installed.
  - [x] uniform themeset with skim: use monokai theme from [jonschlinkert](https://github.com/jonschlinkert/sublime-monokai-extended)
  - [x] event shell & open with terminal: use $TERM if possible, otherwise use the configured terminal application.
        we guess that the user prefers the terminal he's currently using above the configured one. It may change in the future.
  - [x] refactor config creation
  - [x] shorten long names in first line
  - [x] use skim to search for specific line in file
  - [x] manually setup a neovim server with shift+i. Get the server address with `echo serverstart()`.
  - [x] wallpaper aka [nnn](https://github.com/jarun/nnn/blob/master/plugins/wallpaper)
  - [x] bulk: use a menu for rename, files creation, directories creation
  - [x] moc queue management
    - [x] start mocp if not running
    - [x] add file to playlist
    - [x] next, previous song
  - [x] integrate gitroot into shortcuts, remove as a keybinding
  - [x] FIX: trash empty should be confirmed
  - [x] diff of first 2 selected files in second panel

- [ ] Version 0.1.20

  - [ ] edit folder like a buffer [oil like](https://github.com/stevearc/oil.nvim)

- [ ] sub window / menu for completion / selection.

  1. enter selectable mode
  2. chose an action
  3. confirm

- [ ] Version 0.1.50 : safety & memory usage

  - [ ] there's a memory leak somewhere
  - [ ] command::execute in child: prevent it from crashing main app
  - [ ] preview of big files (or whatever file) should only read chunk of the file,
        not the whole thing. Previewing a 5gB iso file uses up to 15gB of ram, which will crash any system.
        or should it ?
        Simple solutions:

        - only reads a buffer
        - limit to xxx filesize

- [ ] Version 0.2.0 : tests

  - [ ] tests
  - [ ] remove references to local thing
  - [ ] translations i18n

- [ ] Version 0.3.0 : dark/light theme, transparency etc.

- [ ] Version 0.4.0 : plugins

  - which language ?
  - what for ?

## BUGS

- [ ] when opening a file with rifle opener into nvim and closing, the terminal hangs
- [ ] log0, log1, log2 are created by log4rs in source folder
  - [x] using absolute path, files are created in the right place
  - [ ] the default file is still `log{}` instead of `log0`...

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

## Sources

### CLI

- [CLI crates](https://lib.rs/command-line-interface)

# Development

I write every step in this file.

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

### Version 0.1.19

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
- [x] Launch NCDU, Lazygit, htop in current folder from a menu with 'S'.

  - [awesome tuis](https://github.com/rothgar/awesome-tuis)

  - [x] Remove lazygit as a separate command
  - [x] Allow configuration from a config file

- [x] display full command before execution
- [x] changing folder (`set_pathcontent`) should set the cwd too... but it has no effect on commands
- [x] FIX: code, subl etc. won't show in exec completion
  - [x] when executable are filtered only files are kept, not symbolink links.
- [x] better error messages when a config file can't be loaded
- [x] messages to display what was made after executing an action

  - [x] improve logging configuration, config from a yaml file moved at build to `$HOME/.config/fm/logging_config.yaml`
  - [x] use 2 separate loggers, normal and for specific actions
  - [x] display logs
  - [ ] log specific actions

    - [x] bulk creation
    - [x] move, copy, delete
    - [x] new dir, new file
    - [x] symlinks creation
    - [x] trash add, trash delete, trash empty

- [x] colors in menus. Use a repeated gradient of lime colors in menus

### Version 0.1.20

- [x] display version in help
- [x] replace FmResult & FmError by anyhow
- [x] update the readme
- [x] replace nvim-send by internal implemention
- [x] mount an iso file by opening it
  - [x] recognize iso files
  - [x] mkdir /run/media/$USER/fm_iso
  - [x] sudo mount -o loop /path/filename.iso /run/media/$USER/fm_iso
- [x] preview the content of a iso file. Require the application isoinfo
- [x] fuzzy finder for keybindings with alt+h. The found keybinding will be run immediatly
- [x] rename: use current name instead of empty string
- [x] don't fail at first error in config.yaml. Allow parsing continuation.
- [x] common trait between cryptdevice and iso_file
- [x] Preview more filetypes - inspired by ranger
  - [x] preview SVG like [ranger](https://github.com/ranger/ranger/pull/2537/files) does
  - [x] preview font with fontimage like [ranger](https://github.com/ranger/ranger/blob/46660c277c2ceb7c7c41ffce794d68f8f559030f/ranger/data/scope.sh#L207-L225)
  - [x] preview doc with pandoc or odt2txt [ranger](https://github.com/ranger/ranger/blob/46660c277c2ceb7c7c41ffce794d68f8f559030f/ranger/data/scope.sh#L84-L93)
  - [x] preview notebooks
- [x] mocp go to song: `mocp -Q %file` with alt+enter (lack of a better keybinding)
- [x] display openers in help

### Version 0.1.21

- [x] more shortcuts like `nnn` : `\` root, @: start
- [x] display settings (dual pane, full display) can be setup in config file.
- [x] common vim keys: require an update of the config file
  - [x] hjkl
  - [x] g G
  - [x] J K C+u C+d
  - [ ] ???
- [x] custom shell command on selection or flagged files, with or without confirmation
- [x] custom action in help
- [x] FIX: absent key in config file can crash the app
- [x] basic shell execution with !
  - [x] completion with which crate
  - [x] shell expansion %e %f etc
- [x] Refactor: use &[] instead of &Vec for arguments in command execution functions.
- [x] Explain every inputsimple mode in second window with static strings
- [x] FIX isodevice:
  - [x] remove useless mode
  - [x] use selected filepath instead of current directory
- [x] after mounting an iso device, move to its mountpoint
- [x] allow generic types for executable in `execute_...` commands
- [x] allow sudo commands from ! actions
  - [x] separate password holder from device action
  - [x] dispatch password
  - [x] execute a command with sudo privileges
- [x] FIX: modification time used `%d/%m/%y`. Changed to `%Y/%m/%d` to allow sorting and respect conventions
- [x] display sort kind in first row
- [x] EventExec refactor
  - [x] event: linked to an Action, same name
  - [x] exec: linked to an executable mode, same name
  - [x] every helper should be moved outside the struct
- [x] FIX: impossible to compile on MacOs since to `sysinfo::Disk` only implement `PartialEq` on linux.
      Can't test MacOs compilation since I don't own a mac...
- [x] FIX: incompatible config files between versions crashes the app.
- [x] FIX: Help string susbtitution aren't aligned properly
- [x] FIX: exiting preview doesn't refresh
- [x] Mode should know if a refresh is required after leaving them.

### Version 0.1.22

- [x] FIX: copying 0 bytes crash progress bar thread
- [x] FIX: refresh users (from tab) reset the selection in pathcontent.
- [x] FIX: when switching from single to dual pane, sort and position are lost
- [x] FIX: tree mode: move down from bottom crashes
- [x] FIX: inxi --full or inxi -F hangs. Use inxi -v 2 instead
- [x] allow shell expansion (~ -> /home/user) in goto mode
- [x] FIX: mode CHMOD reset the file index
- [x] better display of selected tab. Reverse the colors in the first line
- [x] display a message when trash is empty in trash explore mode (alt-o)
- [x] display last executed action (use a string as message)
- [x] FIX: vertical resize to a smaller window : files expand to the last line and message are overwritten
- [x] FIX: open a secondary window and messages are overwritten by files. Don't display messages...
- [x] FIX: clippy term_manager::windmain has too many arguments. Create a struct holding params for winmain
- [x] NeedConfirmation modes are responsible for their confirmation message
- [x] Use Alt+r to remote mount with sshfs.
  - request `username hostname remotepath` in one stroke,
  - execute `sshfs remote_user@hostname:remote_path current_path` which will mount the remote path in current path
- [x] FIX: search keybindings don't work. Need to trim a string.
- [x] FIX: archive depends on CWD and will crash if it's not set properly (ie. change tab, move, change tab, compress)
- [x] use memory and not disk to read last line of logs.
- [x] mocp clear playlist with ctrl+x
- [x] FIX: MOCP print error message on screen
- [x] cryptdevice requires lsblk & cryptdevice. Display a message if not installed
- [x] mocp must be installed to run relatives commands
- [x] nitrogen must be installed to set a wallpaper
- [x] mediainfo must be installed to preview a media file with it
- [x] ueberzug must be installed to preview images & font files with it
- [x] pandoc must be installed to preview doc (.doc, .odb)
- [x] jupyter must be installed to preview .ipynb notebooks.
- [x] isoinfo must be installed to preview .iso files
- [x] diff must be installed to preview a diff of 2 files
- [x] git muse be installed to display a git status string
- [x] inform user if file already exits when creating / renaming
- [x] factorise new file & new dir
- [x] metadata in tree mode. Toggle like display full with alt-e
- [x] FIX: pagedown may select a file outside window without scrolling to it
- [x] FIX: multiple scrolling bugs. It should be smooth in every context
- [x] FIX: after scrolling left click doesn't select the correct file
- [x] FIX: page down when few files in current path screw the display
- [x] remove doublons from shortcut (Ctrl+g) "goto mode"
- [x] FIX: scrolling isn't smooth anymore
- [x] InputSimple is responsible of its help lines
- [x] Preview epub. Requires pandoc.
- [x] FIX: symlink aren't displayed at all.
  - Improve broken symlink detection and display
  - Use `symlink_metadata` to avoid following symlink in tree mode, which may cause recursive filetree
  - Don't display symlink destination in tree mode since it clutters the display
  - Use a different configurable color for broken symlink
- [x] display selected file in first line
- [x] FIX: sort by size use wrong value and order badly 2B > 1M
- [x] refactor copy move. CopyOrMove is responsible for its setup.
- [x] refactor main. Split setup, exit and main loop.
- [x] refactor main. Use a struct responsible for setup, update, display and quit.
- [x] preview fonts, svg & video thumbnail
  - video thumbnail requires ffmpeg
  - fonts preview otf not supported
  - fonts preview requires fontimage
  - svg preview requires rsvg-convert
- [x] preview for special files :
  - [x] block device using lsblk
  - [x] socket using ss
  - [x] fifo & chardevice `lsof path`
- [x] size for char & block device [exa](https://github.com/ogham/exa/blob/fb05c421ae98e076989eb6e8b1bcf42c07c1d0fe/src/fs/file.rs#L331)
- [x] use a struct for ColumnSize
- [x] FIX: goto mode from tree node with a selected file crashes the application
- [x] Not accessible file in tree mode crashes the application
- [x] Look for nvim listen address in `ss -l` output

### Version 0.1.23

- [x] preview tar archive
- [x] Jump mode : 'Space' Toggle flag, 'u' remove all flags, 'Enter' jump to the file
- [x] FIX: copy / move while existing file already exist use another name
- [x] Jump mode (display flagged files) should allow to delete / trash the flagged files
- [x] binary preview also display parsed ASCII strings
- [x] skim fuzzy find (ctrl-f) starts from current dir, not current selected file
- [x] open/file pick flagged files if there are, use selected file instead
- [x] regroup openers when opening multiple files.
- [x] refresh every 10 seconds. If no file in current dir has changed, nothing happens.
- [x] scroll in preview second screen
- [x] FIX sending `Event::User(())` events from refresher hangs skim. Use `Event(Key::AltPageUp)` which is now reserved.
- [x] allow file selection from args : -p filename selects the file from parent dir
- [x] more args : dual pane, preview second, display full, show hidden
- [x] history: when moving back select back the file we were at
- [x] use yellow block char to make flagged files more visibles.
- [x] move input 1 char right since we inserted a space
- [x] preview pdf with ueberzug. First page extracted with poppler -> cairo -> thumbnail -> ueberzug
- [x] FIX: when encrypted drive is already mounted don't let user mount it again
- [x] FIX: group & owner metadata alignement in tree mode
- [x] Tree mode Copy / Move / New should copy in selected directory not root of tree
- [x] Allow scrolling in preview pdf. Required a lot of change in Preview::ueberzug. Update thumbnail when required.
- [x] Flag the selected file if no file is flagged before entering delete mode or trashing a file.
- [x] FIX: fuzzy finder should do nothing if escape (quit?) is inputed
- [x] preview openoffice / office documents as images. Don't use pandoc for .doc .odb etc. previews
- [x] mtp mount with gio [nnn plugin](https://github.com/jarun/nnn/blob/master/plugins/mtpmount)
  - [x] add MTP mount points to shortcuts
  - [x] list, mount, unmount mtp mount points
- [x] bulk, skim & removable are `None` until first use.
- [x] remove dependencies
- [x] complete refactor of many files.
- [x] Use `lazy_static` to load `Colors` configuration. Don't use a cache. Calculate every color for every extension
- [x] allow rgb colors in config file
- [x] FIX: can't read filename from / ... which crashes the app.
- [x] FIX: exploring root folder leads to wrong first line display.
- [x] allow seveval palettes for normal file colors
- [x] move every lazy_static configuration into config.
- [x] FIX: encrypted are never shown as mounted
- [x] Tree remade without recursion. Use an `HashMap<PathBuf, Node>`
  - [x] FIX: folders are max depth hangs the app
  - [x] FIX: rename renames the root path
  - [x] FIX: scrolling to bottom of tree is bugged
  - [x] FIX: scrolling starts 1 row to low
  - [x] FIX: filename in first line
  - [x] FIX: can't "open" a folder to redo the tree there
  - [x] FIX: move back from root should redo the parent tree
  - [x] FIX: move up from to go to last and vice versa
  - [x] FIX: enter a dir from normal mode shouldn't set mode tree
  - [x] Use a generic trait for movements
  - [x] FIX: first line position for tree
  - [x] FIX: searching for file very low don't scroll there
  - [x] FIX: search can only find the first match
  - [x] FIX: leaving preview doesn't reset tree
  - [x] Add a link to previous and next node in Node. Simplify navigation, increase ram usage :/
  - [x] test everything
  - [x] refactor
  - [x] document

### Version 0.1.24

#### Summary

- New Context Menu (right click, Alt+t) with basic file operations.
- Header (path, filename), Footer (other informations).
- Every window can be clicked. Header, Footer, Files, Menus. Selectable element from menu can be clicked.
- Integrated a lot of commands into `TuiApplications` or `CliApplications`.
- Session for display settings. Settings are saved after each modification.
- Better (?) keybindings. Alt+char open a menu whose name starts with this char.
- Refactoring `lib`. Moved file to a few folders, separated display from status.
- Many bug fixes

#### Changelog

- [x] refactor term manager. Separate content construction from drawing.
- [x] better messages when asking a password
- [x] FIX: trash is buggy. Can't delete definitely. Display is wrong when empty.
- [x] FIX: cursor is off by one in password
- [x] display mode / edit mode. Separate display (normal, tree, preview) from any other mode.
- [x] FIX: NVIM listen address won't update if neovim is restarted while fm is still running
- [x] FIX: next is wrong when folded.
      Needs a lot of change. Can't fix everything. ATM if a childnode is folded, unfolding also unfolds every child.
      IDK how to avoid that without rewriting everything.

      We need to do next on node until we reach a displayed node. It's not good.

- [x] separate display_modes completely. Normal -> lsl (?), Tree, Preview.
      PathContent is only used in Normal and should be associated with it.
      Reseting display should switch back to user setted display.
      Preview isn't like Normal & Tree since it doesn't display files at all.
  - [x] move every src to a related folder
- [x] refactor regex. Allow regex match in tree
- [x] FIX: can't jump to file from tree mode
- [x] refactor chmod into its own mod
- [x] refactor node creation (file or directory) in its own mod
- [ ] refactor password.
  - [x] only one enum
- [x] FIX: browsing a file where "modified" time is in future crashes the application.
      The error comes from refresher.rs and happens with poorly configured iso files.
- [x] Fix the error itself,
- [x] log a message when we encounter such a file, since there's not much we can do
- [x] prevent logging the same file multiple times. Massive change which requires a new lazystatic element
- [x] regroup commands in one place
- [x] FIX: clicking outside above files crashes
- [x] Clickable first line. Each first line element can be clicked, resulting in an action
- [x] don't allow rename of parent or self folder (. & ..)
- [x] FIX: print on quit requires to drop everything holding a terminal
- [x] Improve refresh actions. Subfolders in tree mode are now watched.
- [x] Trait LineDisplay for EditMode
- [x] Split tree & lsl drawing into smaller methods
- [x] cargo clippy -- -D clippy::pedantic -D clippy::nursery
      1100 -> 891 errors
- [x] FIX: resizing deselects the current file.
- [x] FIX: first line use selected tab even when not selected
- [x] refactor bulk. Made it a bit simpler
- [x] status refactoring. Moved every "menu" to a separate file.
- [x] FIX: leaving edit mode doesn't refresh completion
- [x] use rc instead of owned types in fileinfo
- [x] renamed path_content to Directory
- [x] renamed Display::Normal to Display::Directory
- [x] event_dispatch refactor
- [x] refactor file opening. Far from satisfying
- [x] FIX: wrong position of cursor in edit modes. Use a cursor offset for all modes.
- [ ] merge specific commands & cli_info
  - [x] merge
  - [x] remove specific commands
- [x] don't store as much info while parsing tree content. Be as lazy as possible.
- [x] Context menu (right click, alt+t) with most common actions
- [x] unify keybindings: alt+char should be reserved to menus starting with same letter
- [x] Clickable layout
  - [x] 1 row : address & file
  - [x] 50% or 100% : files
  - [x] 50% or 0% : menu
  - [x] last row : infos with fixed sizes
- [x] Session with display settings like [felix](https://github.com/kyoheiu/felix/blob/main/src/session.rs)
  - [x] display settings aren't read from args or config file except for "display all".
  - [x] read (using serde_yaml) and write (using serde::Serialize) display settings from a [session file](~/.config/fm/session.yaml).
  - [x] make every session element private, ensure we read the correct setting for dual.
- [x] FIX: opening help or fuzzyfindhelp crashes if a listed action has no keybind (aka. the user overwritten a keybind without creating one for old action).
- [x] !command & cli_application print the command run and the output
- [x] Pre release
  - [x] Fix last missing items
  - [x] check installation (remove config, cargo build)
  - [x] readme
  - [x] describe what was done succintly
  - [x] test every mode

## Current dev

### Version 0.1.25

#### Summary

- Improve scrolling in tree mode.
- Bulk & normal rename: create intermediate folders when renaming.
- Bulk now asks a confirmation before execution.
- Scroll to selection when opening a menu. Selected file should alway be displayed.
- Scroll completion & navigation menus.
- Configurable cli applications. See `~/fm/cli.yaml`
- Simplify tui config files. Remove `cwd` boolean parameter. Doesn't break old config files.
- Display the number of entries as the size of a directory instead of '-'. Might affect performance for very large directories.
- Enable logging with `-l` or `--log`. Nothing is logged anywhere without this flag.
- FIX: Tree mode. Unfolding a directory unfold its children
- BREAKING: Use specific argument to run a command at startup for most common terminal emulators from config file.
  To make this work, it will require the user to update its config file by copying the last part: "terminal_emulator_flags:..."
- FIX: entering an inaccessible dir or writing in any way to a readonly dir shouldn't crash the app anymore...
- Display & Update separation. Use a thread for display, allowing 30 fps. It uses more CPU :(

#### Changelog

- [ ] unify scrolling.
- [x] move navigate movement to menu file.
- [x] Tree scrolling
  - [x] attach full content to tree
  - [x] update content when tree necessary (???)
  - [x] attach window to content
  - [x] FIX: smoothscrolling. Last tree line isn't displayed
  - [x] jump to next sibling
  - [x] compare memory usage
  - [x] scrolling in tree mode should start 1 line earlier since we use 1 line at the bottom
  - [x] FIX: Tree mode: search forward doesn't scroll
- [x] make navigable content scrollable
- [x] leaving second pane as preview should set second pane to normal
- [x] FIX: in tree mode, refresh also refresh the window. Incorrect number of file displayed
- [x] FIX: while second window is opened, if the selection is below half screen, it's not shown anymore.
- [x] exec multiple flagged files
- [x] confirm before bulkaction
- [x] allow bulk rename and normal rename to move files + bulk refactoring
- [x] nohup, nix, setsid ???
  - [x] replace nohup by setsid
  - [x] check for setsid in path and use normal child if it's not.
- [x] cli info is configurable
- [x] refactor menus. split selectable content trait into 2 traits. Use closure to impl methods
- [x] refactor cli & tui applications using common traits. Simplify tui config file
- [x] rename Action::Command to Action::Action since it's what it does
- [x] use number of files for dir size
- [x] rename Goto Mode to cd to allow `:` then `cd`
- [x] optionable logging
- [x] FIX: Filter isn't shown and does nothing in tree mode
- [x] FIX: unfold shouldn't unfold every child
- [x] don't use -e for every terminal. See [rifle.conf](https://github.com/ranger/ranger/blob/136416c7e2ecc27315fe2354ecadfe09202df7dd/ranger/config/rifle.conf#L244)
- [x] FIX: preview a symlink crashes the app
- [x] FIX: opening an inaccessible dir crashes the app
  - [x] check std set env crashes before
  - [x] check all writes
    - [x] Rename,
    - [x] Copy, Move,
    - [x] Delete,
    - [x] Trash, Untrash,
    - [x] Compress,
    - [x] Decompress
- [ ] navigating in menus erase display - can't reproduce ?
- [ ] separate display from update
  - [x] make status sync: replace Rc by Arc
  - [x] update status every event
  - [x] display 30 fps
  - [x] move display into a thread
  - [ ] test everything
- [ ] display number of matches while searching
- [ ] search result
  - [ ] another display mode
  - [ ] use skim or other
  - [ ] action on all files

## TODO

- [ ] refactor & unify all shell commands
- [ ] config loading : https://www.reddit.com/r/rust/comments/17v65j8/implement_configuration_files_without_reading_the/
- [ ] mount usb key - should be merged with mtp
- [ ] document filepicking (from my config etc.).
- [ ] avoid multiple refreshs if we edit files ourself
- [ ] remote control

  - [ ] listen to stdin (rcv etc.)
    - [ ] follow change directory
    - [ ] when called from a file buffer in nvim, open with this file selected
  - [ ] nvim plugin - set a serverstart with a listenaddress, send it to fm
  - https://github.com/KillTheMule/nvim-rs/blob/master/examples/basic.rs
  - https://neovim.io/doc/user/api.html

- [ ] zoxide support
- [ ] temporary marks
- [ ] context switch
- [ ] read events from stdin ? can't be done from tuikit. Would require another thread ?
- [ ] pushbullet ?

- [ ] update the animation

- [ ] build option to force reset of config file, warn the user at first start
- [ ] edit folder like a buffer [oil like](https://github.com/stevearc/oil.nvim)
- [ ] allow pipe in execution

- [ ] tests
- [ ] remove references to local thing
- [ ] translations i18n
- [ ] dark/light theme, transparency etc.
- [ ] plugins

  - which language ?
  - what for ?

## Won't do

### auto stuff

All of this stuff can be done easily through a shell command or automatically. I'm not sure I wan't to bloat fm with it.

- [ ] auto mount usb keys ??? just use udiskie (started automatically) and udiskie-umount /mount/point
- [ ] cloud services (apple, microsoft, google, dropbox etc.)
- [ ] ftp

## Sources

### CLI

- [CLI crates](https://lib.rs/command-line-interface)

### Chat GPT ideas

Tell me 10 cool features a TUI file manager could have

Sure! Here are 10 cool features a TUI (Text-based User Interface) file manager
could have:

Easy navigation: A TUI file manager should allow users to navigate through
directories and files quickly and easily using keyboard shortcuts.

File previews: Users can preview the contents of a file without having to open
it. This feature can be particularly useful for large text files, images, or
videos.

Customizable interface: A TUI file manager should be customizable so that users
can choose the information they want to see on the screen, such as file size,
modification date, and file type.

File operations: Basic file operations like copying, moving, and deleting
should be easily accessible and efficient to perform.

Bulk operations: A TUI file manager should allow users to perform bulk
operations on multiple files at once, such as renaming or deleting them.

Tabbed interface: A tabbed interface allows users to have multiple directories
open at the same time, making it easy to move files between them.

Bookmarking: Users can bookmark frequently used directories for quick access.

Integrated terminal: A TUI file manager can include an integrated terminal to
execute commands without switching to another application.

Search functionality: Users can search for files based on their name, content,
or other criteria, with the search results displayed in real-time.

Cloud storage integration: TUI file managers can integrate with cloud storage
services like Dropbox, Google Drive, or OneDrive, allowing users to manage
their cloud files directly from the file manager interface.

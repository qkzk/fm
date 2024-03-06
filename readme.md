# FM: a file manager inspired by dired and ranger, written in rust

[![fm-tui on crates.io][cratesio-image]][cratesio]
[![fm-tui on docs.rs](https://img.shields.io/docsrs/fm-tui/0.1.24)][docrs]

[cratesio-image]: https://img.shields.io/crates/v/fm-tui.svg
[cratesio]: https://crates.io/crates/fm-tui
[docsrs-badge]: https://img.shields.io/docsrs/fm-tui/0.1.24
[docrs]: https://docs.rs/fm-tui/0.1.24

```
A TUI file manager inspired by dired and ranger

Usage: fm [OPTIONS]

Options:
  -p, --path <PATH>      Starting path. directory or file [default: .]
  -s, --server <SERVER>  Nvim server [default: ]
  -A, --all              Display all files (hidden)
  -l, --log              Enable logging
      --neovim           Started inside neovim terminal emulator
  -h, --help             Print help
  -V, --version          Print version                                                                                                                                                                              [3,8s]
```

## Platform

Linux is the only supported platform. It may be usable on MacOS but I can't be sure.

## Video

![fm](https://github.com/qkzk/fm/blob/v0.1.23-dev/fm.gif?raw=true)

## Installation

```sh
cargo install fm-tui
```

## Usage

Start it from command line with no parameters :

```sh
fm
```

or with a path :

```sh
fm -p ~/Downloads
```

If you added the [recommanded function](#cd-on-quit) to your bashrc/zshrc, simply use `f` and you will cd to the last visited directory when exiting.

## Features

Some features depends on external programs to keep fm from being really bloated.
I try to implement every feature I can think of.

### Session

Display settings (use two panes, display metadata, use second pane as preview) are saved and restored when restarting the application.

### Navigation

- Navigate with the arrows or the mouse (left select, right open, wheel)
  Basic vim keys are supported by default: hjkl, gG, Ctrl+U Ctrl+D, JK
- Open a file with o, enter or right click
- Execute a file with a custom command with e

### Moving

Many ways to jump somewhere :

- Alt+g: type the full address (with completion enabled),
- Ctrl+g: a predefined shortcut (default root folders, home and mount points, gitroot, config folder),
- Alt+j: by jumping to a flagged file,
- ': by creating your own marks and jumping to them

### File manipulation

- Flag files with `space` (\*: flag all, v: reverse, u: unflag)
- Copy / move / symlinks / delete / trash flagged files with c, p, s, x, X
- Create files, directory, rename with n, d, r
- Flag a bunch of file, change panel with TAB and move/copy them !
- Open the trash with Alt+o. x to remove permanently, enter to restore. Wipe the trash with Alt+x.
- Rename or create a bunch of file with alt-b. Flag files, alt-b, edit the names and save the file. The renaming is done.
  You can create nested files with `a/b/c` which will create every intermediate folder if needed.

### Shell

- Open a new shell in this directory with s
- Start a configured TUI application with alt-s (like htop, ncdu etc.)
- Start a configured CLI application with alt-i (like diff, dragon-drop etc.)
- Execute a shell command with '!'. Expansions (%e ext, %n filename, %s filepath, %f flagged files, %d current directory) are supported.
  pipes and redirections aren't supported.

### Display

- Change display, removing details with E or display a single pane with Alt+d
- Preview most of files (text, highlighted code, binary, pdf, exif details, image/video, audio details, archives, MS-office & OpenOffice documents) with P
- Toggle the tree view with t. Fold selected folder with z. Unfold every folder with Z, fold every folder with Alt+z.
- Enter preview mode with Alt+P. Every file is previewed in the second pane.
- Filter the view (by extension, name, directory only, all files) with F
- Find files with / (with completion: Tab, enter to search),
- flag files matching a regex with w

### Fuzzy finders

- Ctrl-f : search in filenames and move there,
- Ctrl-s : search for a line in file content and move there,
- H : display a searchable help, search for a keybinding and execute the action.

We use a fork of [skim](https://github.com/lotabout/skim), an fzf clone written in rust.

### Neovim filepicker

When you open a file with i, it will send an event to Neovim and open it in a new buffer.

As long as Neovim is runnging, it should always work, even outside of neovim.

The RPC server address is found by looking for neovim in /proc. If it fails, we can still look for an
environment variable set by neovim itself.
Finally, it's also possible to pass the RPC server address with `fm -s address`.

### cd on quit

When leaving fm, it prints the last visited path.
If you add this function to your `zshrc` / `bashrc`, it will listen to stdout and cd to the last dir.

```bash
function f() {
  # start the fm filemanager, enabling cd on quit.
  dest=$(fm $@)
  if [[ ! -z $dest ]]
  then
   cd $dest
  fi
}
```

For fish users, this is the function to add to your `config.fish`

```bash
function f
  # start the fm filemanager, enabling cd on quit.
  set dest (fm $argv)
  if not test -z $dest
    cd $dest
  end
end
```

### Archives

- Decompress an archive by opening it (o, enter, right click)
- Compress flagged files with C. Pick the desired algorithm from a menu.

### Custom binds

You can bind any _unbound_ key to a shell command.

- pipe & redirections (| > >> <) aren't supported !
- the first word you type is the executable. Don't start your command with environment variables, it won't work.

Expansions :

- %e : extension
- %s : selected file (full path)
- %f : flagged files (full path)
- %n : selected filename
- %d : current directory

### Logging

With `-l` or `--log`, logs are enabled. They are disabled by default.

Critical actions will be logged to `~/.config/fm/log/fm.log` and actions affecting the file tree will be logged to `~/.config/fm/log/action_logger.log`.

The last action is displayed at the bottom of the screen and can be read with `Alt+l` like a preview.
Those logs can be seen even if logging is disabled, it just won't be up to date.

### More

- Copy a filename/filepath to clipboard with Ctrl+n, Ctrl+p
- Detect removable disks automatically and jump to them in a few keystrokes (Ctrl+g, up, enter)
- Drag and drop files (requires dragon-drop installed) with D
- Open and mount encrypted devices. Open the menu with Shift+e, mount with m, unmount with u.
- Contol MOCP with Ctrl+arrows. Ctrl+Left, Ctrl+Right: previous or next song. Ctrl+Down: Toggle pause. Ctrl+Up: add current folder to playlist
- Set the selected image as wallpaper with W.
- Enter "command mode" with ':'. Type the name of a command and it will be executed.
- Mount a remote filesystem using ssfhs with Alt-r.
- Mount a MTP device with Alt-R.

Most of those features are inspired by ranger and alternatives (Midnight commander, nnn, lf etc.), the look and feel by dired.

## Default keybindings

Press ctrl-h to display the help.
Your current keybindings are shown. Here are the default ones.

```
Char('q') :      quit
Ctrl('h') :      help

- Navigation -
Left      :      cd to parent directory
Char('l') :      cd to child directory
Up        :      one line up
Char('j') :      one line down
Home      :      go to first line
Char('G') :      go to last line
PageUp    :      10 lines up
Char('J') :      10 lines down
Tab       :      cycle tab

- Actions -
Alt('d')  :      toggle dual pane - if the width is sufficiant
Alt('p')  :      toggle a preview on the second pane
Char('E') :      toggle metadata on files
Char('a') :      toggle hidden
Char('s') :      shell in current directory
Char('o') :      open the selected file with :
    - default       xdg-open
    - audio         mocp
    - images        viewnior
    - office        libreoffice
    - pdf, ebooks   zathura
    - text          nvim
    - video         mpv
    - vectorials
    - compressed files are decompressed
    - iso images are mounted
Char('i') :      open in current nvim session
Char('I') :      setup the nvim rpc address
Char('P') :      preview this file
Char('-') :      move back to previous dir
Char('~') :      move to $HOME
Char('`') :      move to root (/)
Char('@') :      move to starting point
Char('M') :      mark current path
Char('\''):      jump to a mark
Char('f') :      search next matching element
Ctrl('f') :      fuzzy finder for file
Ctrl('s') :      fuzzy finder for line
Char('H') :      fuzzy finder from help
Ctrl('r') :      refresh view
Ctrl('c') :      copy filename to clipboard
Ctrl('p') :      copy filepath to clipboard
Alt('c')  :      open the config file

- Action on flagged files -
Char(' ') :      toggle flag on a file
Char('*') :      flag all
Char('u') :      clear flags
Char('v') :      reverse flags
Char('L') :      symlink to current dir
Char('c') :      copy to current dir
Char('m') :      move to current dir
Char('x') :      delete files permanently
Char('X') :      move to trash
Char('C') :      compress into an archive

- Trash -
Alt('o')  :      Open the trash (enter to restore, del clear)
Alt('x')  :      Empty the trash

- Tree -
Navigate as usual. Most actions works as in 'normal' view.
Char('t') :      Toggle tree mode
Char('z') :      Fold a node
Ctrl('z') :      Fold every node
Char('Z') :      Unfold every node

    - DISPLAY MODES -
Different modes for the main window
Ctrl('q') :      NORMAL
Char('t')  :      TREE
Char('F') :      FLAGGED
Char('P') :      PREVIEW

    - EDIT MODES -
Different modes for the bottom window
Alt('m')  :      CHMOD
Char('e') :      OPEN WITH
Char('d') :      NEWDIR
Char('n') :      NEWFILE
Char('r') :      RENAME
Alt('g')  :      CD
Char('w') :      REGEXMATCH
Alt('j')  :      JUMP
Char('O') :      SORT
Alt('h')  :      HISTORY
Ctrl('g') :      SHORTCUT
Alt('e')  :      ENCRYPTED DRIVE
    (m: open & mount,  u: unmount & close, g: go there)
Alt('R')  :      REMOVABLE MTP DEVICES
    (m: mount,  u: unmount, g: go there)
Char('/') :      SEARCH
Char(':') :      ACTION
Alt('b')  :      BULK
Alt('s')  :      TUI APPS
Alt('i')  :      CLI APPS
Alt('r')  :      MOUNT REMOTE PATH
Alt('f')  :      FILTER
    (by name "n name", by ext "e ext", "d only directories" or "a all" for reset)
Enter     :      Execute mode then NORMAL

- MOC -
Control MOC from your TUI
CtrlUp    :      MOCP: Add selected file or folder to the playlist
CtrlLeft  :      MOCP: Previous song
CtrlDown  :      MOCP: Toggle play/pause.
CtrlRight :      MOCP: Next song
AltEnter  :      MOCP: Go to currently playing song
Ctrl('x') :      MOCP: Clear the playlist

- CUSTOM ACTIONS -
%s: the selected file,
%f: the flagged files,
%e: the extension of the file,
%n: the filename only,
%p: the full path of the current directory.
Alt('u'):        /usr/bin/google-chrome-stable %s
Char('D'):        /usr/bin/dragon-drop %s
```

## Configuration

Every configuration file is saved in `~/.config/fm/`

You can configure :

- **Keybindings**. Some should be left as they are, but all keybindings can be configured.
  use the provided config file as a default.
  Multiple keys can be bound the the same action.
- **Custom actions**. You can bind any key to a shell command.
  - don't use pipes or redirectons, they won't be parsed correctly
  - use an unset bind
  - %s is expanded to the selected path, %f is expanded to the flagged files (full paths).
  - See the [config](./config_files/fm/config.yaml) or an example.
- **Openers**. fm tries to be smart and open some files with a standard program.
  You can change that and use whatever installed program you want. Specify if it
  requires a shell to be run (like neovim) or not (like subl).
- **Marks**. Users can save about 100 differents marks to jump to, they're saved
  in your marks.config file. It's easier to let fm manage your marks, but if
  you made a mess or want to start over, simply delete the file or a single line.
- **TUI applications**. Some classic TUI applications like htop, glances, btop, lazygit are already there.
  Open the menu with `S` and pick the desired one. It will only work with a TUI application like HTOP,
  not a CLI application like bat.
- **Colors** of files.
  Non standard files (directory, char devices, block devices, symlinks, sockets, fifo) have their own configurable colors.
  You can use ansi colors or rgb values.
  Standard files are colored by their extension and you can use 3 differents palettes (red-green, red-blue or green-blue).
  Every extension has its own random color.

## External dependencies

Most of the openers and tui applications are configurable from config files. Some are hardcoded if their command is quite specific or if I couldn't find a workaround.

- [lsblk](https://linux.die.net/man/8/lsblk): list encrytped devices
- [faillock](https://linux.die.net/man/8/faillock): reset failed sudo attempts
- [Cryptsetup](https://gitlab.com/cryptsetup/cryptsetup): decrypt & mount encrypted devices
- [Nitrogen](https://github.com/l3ib/nitrogen/): set up a wallpaper
- [MOC](https://moc.daper.net/) Music On Console allows you to play music from your terminal
- [Dragon-Drop](https://github.com/mwh/dragon) drag-and-drop a file from a terminal to a GUI application.
- [Ueberzug](https://github.com/LalleSX/ueberzug) display images in your terminal. Used to preview images. This one may be tricky to install from source since the original maintener nuked his project. It's still available in many package managers.
- [isoinfo](https://command-not-found.com/isoinfo) allow the content preview of an iso file
- [jupyter](https://jupyter.org/) preview jupyter notebooks by converting them to markdown
- [pandoc](https://pandoc.org) preview epub by converting them to markdown with pandoc
- [fontimage](https://fontforge.org/docs/fontutils/fontimage.html) preview fonts by creating a thumbnail
- [rsvg-convert](https://github.com/brion/librsvg) preview svg by creating a thumbnail
- [libreoffice](https://www.libreoffice.org) preview open & MS-office documents

## Contribution

Any help is appreciated.

I comment everything I do in [dev.md](development.md).

It's my first "published" program, so don't get upset by the code quality.

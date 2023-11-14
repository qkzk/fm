# FM: a file manager inspired by dired and ranger, written in rust

[![fm-tui on crates.io][cratesio-image]][cratesio]
[![fm-tui on docs.rs](https://img.shields.io/docsrs/fm-tui/0.1.23)][docrs]

[cratesio-image]: https://img.shields.io/crates/v/fm-tui.svg
[cratesio]: https://crates.io/crates/fm-tui
[docsrs-badge]: https://img.shields.io/docsrs/fm-tui/0.1.0
[docrs]: https://docs.rs/fm-tui/0.1.0

```
A TUI file manager inspired by dired and ranger

Usage: fm [OPTIONS]

Options:
  -p, --path <PATH>      Starting path. directory or file [default: .]
  -s, --server <SERVER>  Nvim server [default: ]
  -D, --dual <DUAL>      Dual pane ? [possible values: true, false]
  -S, --simple <SIMPLE>  Display files metadata ? [possible values: true, false]
  -P, --preview          Use second pane as preview ? default to false
  -A, --all              Display all files (hidden)
  -h, --help             Print help
  -V, --version          Print version
```

## Platform

Linux is the only supported platform.

- Version 0.1.20 doesn't compile on MacOS (see [#77](https://github.com/qkzk/fm/issues/77)).
- Version 0.1.21 fixes this bug but I can't test more since I don't own a mac :)

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
- Rename or create a bunch of file with B. Flag files, B, edit the names and save the file. The renaming is done.
  You can create nested files with `a/b/c` which will create every intermediate folder if needed.

### Shell

- Open a new shell in this directory with s
- Start a configured TUI application with S
- Execute a shell command with '!'. Expansions (%e ext, %n filename, %s filepath, %f flagged files, %d current directory) are supported.
  pipes and redirections aren't supported.

### Display

- Change display, removing details with Alt+e or display a single pane with Alt+d
- Preview most of files (text, highlighted code, binary, pdf, exif details, image/video, audio details, archives, MS-office & OpenOffice documents) with P
- Toggle the tree view with t. Fold selected folder with z. Unfold every folder with Z, fold every folder with Alt+z.
- Enter preview mode with Alt+P. Every file is previewed in the second pane.
- Filter the view (by extension, name, directory only, all files) with F
- Find files with / (with completion: Tab, enter to search),
- flag files matching a regex with w

### Fuzzy finders

- Ctrl-f : search in filenames and move there,
- Ctrl-s : search for a line in file content and move there,
- Alt-h : search for a keybinding and execute the action.

We use a fork of [skim](https://github.com/lotabout/skim), an fzf clone written in rust.

### Neovim filepicker

When you open a file with i, it will send an event to Neovim and open it in a new buffer.

It should always work, even outside of neovim.

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

### More

- Copy a filename/filepath to clipboard with Ctrl+n, Ctrl+p
- Detect removable disks automatically and jump to them in a few keystrokes (Ctrl+g, up, enter)
- Drag and drop files (requires dragon-drop installed) with Alt+d
- Open and mount encrypted devices. Open the menu with Shift+e, mount with m, unmount with u.
- diff the first two flagged files / folders with D.
- Contol MOCP with Ctrl+arrows. Ctrl+Left, Ctrl+Right: previous or next song. Ctrl+Down: Toggle pause. Ctrl+Up: add current folder to playlist
- Set the selected image as wallpaper with W.
- _Experimental_ enter "command mode" with ':'. Type the name of a command and it will be executed.
- Mount a remote filesystem using ssfhs with Alt-r.
- Mount a MTP device with Alt-R.

Most of those features are inspired by ranger and alternatives (Midnight commander, nnn, lf etc.), the look and feel by dired.

## Default keybindings

Press `h` by default to display the help.
Your current keybindings are shown. Here are the default ones.

```


     Char('q'):      quit
     Char('h'):      help

     - Navigation -
     Left:           cd to parent directory
     Right:          cd to child directory
     Up:             one line up
     Down:           one line down
     Home:           go to first line
     End:            go to last line
     PageUp:         10 lines up
     PageDown:       10 lines down
     Tab:            cycle tab

     - Actions -
     Alt('f'):      toggle dual pane - if the width is sufficiant
     Alt('p'):       toggle a preview on the second pane
     Alt('e'):      toggle metadata on files
     Char('a'):      toggle hidden
     Char('s'):      shell in current directory
     Char('o'):      open the selected file
     Char('i'):      open in current nvim session
     Char('I'):      setup the nvim rpc address
     Char('P'):      preview this file
     Char('T'):       display infos about a media file
     Char('-'):      move back to previous dir
     Char('~'):      move to $HOME
     Char('M'):      mark current path
     Char('\''):     jump to a mark
     Char('f'):      search next matching element
     Ctrl('f'):      fuzzy finder
     Ctrl('s'):      fuzzy finder for line
     Ctrl('r'):      refresh view
     Ctrl('c'):      copy filename to clipboard
     Ctrl('p'):      copy filepath to clipboard
     Alt('d'):       dragon-drop selected file
     Alt('c'):       open the config file
     Char('W'):      set the selected file as wallpaper with nitrogen

     - Action on flagged files -
     Char(' '):      toggle flag on a file
     Char('*'):      flag all
     Char('u'):      clear flags
     Char('v'):      reverse flags
     Char('l'):      symlink to current dir
     Char('c'):      copy to current dir
     Char('p'):      move to current dir
     Char('x'):      delete files permanently
     Char('X'):      move to trash
     Char('C'):      compress into an archive
     Char('D'):      display the diff of the first 2 flagged files

     - Trash -
     Alt('o'):       Open the trash (enter to restore, del clear)
     Alt('x'):       Empty the trash

     - Tree -
     Navigate as usual. Most actions works as in 'normal' view.
     Char('t'):      Toggle tree mode
     Char('z'):      Fold a node
     Alt('z'):       Fold every node
     Char('Z'):      Unfold every node

     - MODES -
     Char('t'):      TREE
     Char('m'):      CHMOD
     Char('e'):      EXEC
     Char('d'):      NEWDIR
     Char('n'):      NEWFILE
     Char('r'):      RENAME
     Char('g'):      GOTO
     Char('w'):      REGEXMATCH
     Char('j'):      JUMP
     Char('O'):      SORT
     Char('H'):      HISTORY
     Char('G'):      SHORTCUT
     Char('E'):      ENCRYPTED DRIVE
         (m: open & mount,  u: unmount & close)
     Char('/'):      SEARCH
     Char(':'):      COMMAND
     Char('B'):      BULK
     Char('S'):      SHELL MENU
     Char('F'):      FILTER
         (by name "n name", by ext "e ext", only directories d or all for reset)
     Enter:  Execute mode then NORMAL
     Ctrl('q'):    NORMAL

     - MOC -
     Control MOC from your TUI
     CtrlUp:          Add a file or folder to the playlist
     CtrlLeft         Previous song
     CtrlDown:        Toggle play/pause. Start MOC if needed
     CtrlRight        Next song
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
- **Settings**. Do you whish to start with dual pane ? Do you wish to use basic or
  full display ?
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

# FM: a dired inspired TUI file manager

Written in rust.

[![fm-tui on crates.io][cratesio-image]][cratesio]
[![fm-tui on docs.rs](https://img.shields.io/docsrs/fm-tui/0.1.0)][docrs]

[cratesio-image]: https://img.shields.io/crates/v/fm-tui.svg
[cratesio]: https://crates.io/crates/fm-tui
[docsrs-badge]: https://img.shields.io/docsrs/fm-tui/0.1.0
[docrs]: https://docs.rs/fm-tui/0.1.0

```
FM : dired like file manager


Usage: fm [OPTIONS]

Options:
  -p, --path <PATH>      Starting path [default: .]
  -s, --server <SERVER>  Nvim server [default: ]
  -h, --help             Print help information
  -V, --version          Print version information
```

## Video

![fm](./fm.gif)

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

- Navigate with the arrows or the mouse (left select, right open, wheel)
- Open a file with o, enter or right click
- Execute a file with a custom command with e
- Copy / move / symlinks / delete with c, p, s, x
- Create files, directory, rename with n, d, r
- Open a new shell in this directory with s
- Flag a bunch of file, change panel with TAB and move/copy them !
- Many ways to jump somewhere :

  - g: type the full address (with completion enabled),
  - G: a predefined shortcut (default root folders, home and mount points),
  - j: by jumping to a flagged file,
  - ': by creating your own marks and jumping to them

- Change display, removing details or displaying a single pane.
- Preview most of files (text, highlighted code, binary, pdf, exif details, video/audio details, archives) with P
- Display a tree view of directory by previewing it
- Decompress an archive by opening it (o, enter, right click)
- Copy a filename/filepath to clipboard with Ctrl+n, Ctrl+p
- Rename a bunch of file with B. Flag files, B, edit the names and save the file. The renaming is done.
- Use the integrated fuzzy finder (forked version of skim, an fzf clone) with Ctrl+f to navigate quickly
- Filter the view (by extension, name, directory only, all files) with F
- Find files with / (with completion), flag files matching a regex with w
- Detect removable disks automatically and jump to them in a few keystrokes (G, up, enter)
- Drag and drop files (requires dragon-drop installed) with Alt+D
- Trash a file with X, open the trash with Alt+o. x to remove permanently, enter to restore. Wipe the trash with Alt+x.

Most of those features are inspired by ranger and alternatives (Midnight commander), the look and feel by dired.

## Neovim filepicker

When you open a file with i, it will send an event to Neovim and open it in a new buffer.
Recent versions of neovim export the RPC server address to an environement variable which is read if no argument
is provided.

It should always work, even outside of neovim.

It's also possible to pass the RPC server address with `fm -s address`.

This feature requires `nvim-send` to be installed (`cargo install nvim-send`)

## cd on quit

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

## Default keybindings

Press `h` by default to display the help.
Your current keybindings are shown. Here are the default ones.

```
fm: a dired like file manager. Keybindings.

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
     Char('D'):      toggle dual pane - if the width is sufficiant
     Char('a'):      toggle hidden
     Char('s'):      shell in current directory
     Char('o'):      open the selected file
     Char('i'):      open in current nvim session
     Char('P'):      preview this file
     Char('T'):      display a thumbnail of an image
     Char('-'):      move back to previous dir
     Char('~'):      move to $HOME
     Char('M'):      mark current path
     Char('\''):     jump to a mark
     Ctrl('e'):      toggle metadata on files
     Char('f'):      Search next matching element
     Ctrl('f'):      fuzzy finder
     Ctrl('r'):      refresh view
     Ctrl('c'):      copy filename to clipboard
     Ctrl('p'):      copy filepath to clipboard
     Alt('d'):       dragon-drop selected file

     - Action on flagged files -
     Char(' '):      toggle flag on a file
     Char('*'):      flag all
     Char('u'):      clear flags
     Char('v'):      reverse flags
     Char('l'):      symlink files
     Char('B'):      bulkrename files
     Char('c'):      copy to current dir
     Char('p'):      move to current dir
     Char('x'):      delete files permanently
     Char('X'):      move to trash

     - Trash -
     Alt('o'):       Open the trash (enter to restore, del clear)
     Alt('x'):       Empty the trash

     - MODES -
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
     Char('/'):      SEARCH
     Char('F'):      FILTER
         (by name "n name", by ext "e ext", only directories d or all for reset)
     Enter:  Execute mode then NORMAL
     Ctrl('q'):    NORMAL
```

## Configuration

Every configuration file is saved in `~/.config/fm/`

You can configure :

- **Colors** for non standard file types (directory, socket, char device, block device)
- **Keybindings**. Some should be left as they are, but all keybindings can be configured.
  use the provided config file as a default.
  Multiple keys can be bound the the same action.
- **Openers**. fm tries to be smart and open some files with a standard program.
  You can change that and use whatever installed program you want. Specify if it
  requires a shell to be run (like neovim) or not (like subl).
- **Marks**. Users can save about 100 differents marks to jump to, they're saved
  in your marks.config file. It's easier to let fm manage your marks, but if
  you made a mess or want to start over, simply delete the file or a single line.

## Contribution

Any help is appreciated.

I comment everything I do in [dev.md](development.md).

It's my first "published" program, so don't get upset by the code quality.

use std::io;

use crossterm::cursor::SetCursorStyle;
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect, Size},
};

use crate::config::Bindings;

/// Different states in which the cursor can be.
/// - Inactive: the cursor isn't being used and it should be used,
/// - Movement: the cursor can move but no selection is made,
/// - Selection: the cursor can move and the selection is udapted.
#[derive(Default, Clone, Copy)]
enum CursorState {
    #[default]
    Inactive,
    Movement,
    Selection,
}

impl CursorState {
    fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive)
    }

    pub fn is_selecting(&self) -> bool {
        matches!(self, Self::Selection)
    }

    /// Transition machine:
    /// Inactive -> does nothing;
    /// Movement <-> Selection.
    fn toggle_selection(&mut self) {
        match self {
            Self::Inactive => (),
            Self::Movement => {
                *self = Self::Selection;
            }
            Self::Selection => {
                *self = Self::Movement;
            }
        }
    }

    fn set_active(&mut self) {
        *self = Self::Movement;
    }
}

/// Different direction in which the cursor can move.
#[derive(Default, Clone, Copy)]
pub enum CursorDirection {
    #[default]
    Down,
    Up,
    Left,
    Right,
}

impl CursorDirection {
    fn go_from(&self, x: u16, y: u16) -> Position {
        let mut x = x;
        let mut y = y;
        match self {
            CursorDirection::Down => {
                y = y.saturating_add(1);
            }
            CursorDirection::Up => {
                y = y.saturating_sub(1);
            }
            CursorDirection::Left => {
                x = x.saturating_sub(1);
            }
            CursorDirection::Right => {
                x = x.saturating_add(1);
            }
        }
        Position::new(x, y)
    }
}

/// `Cursor` is used to select and copy or log text directly from fm output.
/// Once the cursor is active, you can't move in the file tree or open menus etc.
/// You can only :
/// - toggle the selection state,
/// - move the cursor with keys or mouse,
/// - copy the selected chars (only while selecting)
/// - leave the cursor and go back to normal usage of fm,
/// - exit fm completely
///
/// It's a way to allow copying text without having to exit fm or open a new shell.
///
/// `Cursor` has a state (inactive, movement, selecting), knows its position and where it started its selection.
/// We also store the associated binds to help the user.
#[derive(Default, Clone)]
pub struct Cursor {
    state: CursorState,
    cursor: Option<Position>,
    origin: Option<Position>,
    rect: Option<Rect>,
    /// Are we dragging the cursor with the mouse ?
    pub is_dragging: bool,
    /// What is the keybind associated to leave menu ?
    pub leave_bind: String,
    /// What is the keybind associated to entering the cursor ? It's used to toggle selection state.
    pub enter_bind: String,
    /// What is the bind associated to copy/paste. Used to copy the selection to clipboard and log it.
    pub copy_bind: String,
}

impl Cursor {
    /// Creates a new cursor with binds read from keybinds.
    pub fn new(binds: &Bindings) -> Self {
        let reversed = binds.keybind_reversed();
        let leave_bind = reversed.get("ResetMode").cloned().unwrap_or_default();
        let enter_bind = reversed.get("Cursor").cloned().unwrap_or_default();
        let copy_bind = reversed.get("CopyPaste").cloned().unwrap_or_default();
        Self {
            state: CursorState::default(),
            cursor: None,
            origin: None,
            rect: None,
            is_dragging: false,
            leave_bind,
            enter_bind,
            copy_bind,
        }
    }

    /// Copy of the inner rect.
    pub fn rect(&self) -> Option<Rect> {
        self.rect
    }

    /// Position of the cursor if any
    pub fn cursor(&self) -> Option<Position> {
        self.cursor
    }

    /// True iff the cursor is in active mode (either movement or selecting)
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// True iff the cursor is selecting.
    pub fn is_selecting(&self) -> bool {
        self.state.is_selecting()
    }

    /// Reset the cursor.
    /// set state to inactive, erase cusrsor, origin & rect and set is_dragging to false.
    pub fn reset(&mut self) {
        self.state = CursorState::default();
        self.cursor = None;
        self.origin = None;
        self.rect = None;
        self.is_dragging = false;
    }

    /// Set the state from inactive to movement or toggle between movement & selecting.
    pub fn toggle(&mut self, position: Position) {
        if self.state.is_active() {
            self.toggle_selection();
        } else {
            self.start_cursor(position);
        }
    }

    /// Set default values for entering selection from this position.
    /// Wattchout: it also changes the _TERMINAL_ cursor to "steady block".
    fn start_cursor(&mut self, position: Position) {
        let _ = crossterm::execute!(io::stdout(), SetCursorStyle::SteadyBlock);
        self.state.set_active();
        self.cursor = Some(position);
        self.origin = Some(position);
        self.rect = None;
    }

    /// Toggle between selecting & movement.
    /// Does nothing if cursor isn't already active.
    fn toggle_selection(&mut self) {
        if !self.state.is_active() {
            return;
        }
        self.state.toggle_selection();
        if self.state.is_selecting() {
            self.origin = self.cursor;
        } else {
            self.clear_selection();
        }
    }

    /// Clear the current selected rect.
    fn clear_selection(&mut self) {
        self.rect = None;
    }

    /// Move the cursor in given direction.
    /// Cursor is clamped to the the screen.
    /// It's coordinates can't be equal or bigger to terminal height or width.
    pub fn move_cursor(&mut self, direction: CursorDirection, Size { width, height }: Size) {
        let Some(Position { x, y }) = self.cursor else {
            return;
        };
        let new_pos = direction
            .go_from(x, y)
            .clamp(Position::ORIGIN, Position::new(width, height));
        self.cursor = Some(new_pos);
    }

    /// Move the cursor to `position`
    /// Does nothing if cursor isn't active.
    pub fn move_cursor_to(&mut self, position: Position) {
        if !self.state.is_active() {
            return;
        }
        self.cursor = Some(position);
    }

    /// Move the origin of selection to `position`
    /// Does nothing if cursor isn't active.
    pub fn move_origin_to(&mut self, position: Position) {
        if !self.state.is_active() {
            return;
        }
        self.origin = Some(position);
    }

    /// Update the selection from origin to current position.
    /// Does nothing if cursor isn't active.
    pub fn extend_selection(&mut self) {
        if !self.state.is_selecting() {
            return;
        }
        let start = self.origin.expect("Should be set");
        let end = self.cursor.expect("Should be set");
        let x = start.x.min(end.x);
        let y = start.y.min(end.y);
        let width = u16::abs_diff(start.x, end.x) + 1;
        let height = u16::abs_diff(start.y, end.y) + 1;
        self.rect = Some(Rect::new(x, y, width, height));
    }

    /// Used to allow selecting text with the mouse.
    /// Either start selecting from previous position if we enter the "dragging" state
    /// Move the cursor to the current mouse position.
    /// Or extend selection.
    pub fn mouse_drag(&mut self, row: u16, col: u16) {
        let pos = Position::new(col, row);
        self.move_cursor_to(pos);
        if self.is_dragging {
            self.extend_selection();
        } else {
            self.move_origin_to(pos);
            self.is_dragging = true;
        }
    }

    /// Stop dragging.
    pub fn stop_drag(&mut self) {
        self.is_dragging = false;
    }
}

/// Returns a string made of the chars from the buffer which are in the given rect.
pub fn read_rect_from_buffer(rect: &Rect, buffer: &Buffer) -> String {
    let mut content = String::new();
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let Some(cell) = buffer.cell((x, y)) else {
                // Should never happen since the rect is inside the displayed window.
                continue;
            };
            content.push_str(cell.symbol());
        }
        content.push('\n')
    }
    content
}

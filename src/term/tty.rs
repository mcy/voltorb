//! Low-level wrappers over a TTY, i.e., a viewport that can display texels
//! and receive keyboard/mouse input.

use std::io;
use std::io::Write as _;
use std::panic;
use std::time::Duration;

use enumflags2::BitFlags;

use crate::term::texel::Color;
use crate::term::texel::Texel;
use crate::term::texel::Weight;

/// An abstraction over an interactive terminal.
///
/// This trait exists primarily for ease of integration testing.
pub trait Tty {
  /// Initializes the terminal into an appropriate mode, such as setting the
  /// "alt window", enabling "raw mode", and configuring receipt of other
  /// events (for a Unixy tty).
  fn init(&mut self) -> io::Result<()>;

  /// Reverses any settings [`Tty::init()`] set, for program exit.
  fn fini(&mut self) -> io::Result<()>;

  /// Returns the width in columns and height in rows of the terminal.
  ///
  /// The returned `Cell` is *not* a valid coordinate for writes!
  fn viewport(&mut self) -> io::Result<Cell>;

  /// Polls for an event from this terminal, potentially timing out.
  ///
  /// Implementations may ignore `timeout`.
  fn poll(&mut self, timeout: Option<Duration>) -> io::Result<Option<Event>>;

  /// Writes a run of texels to this terminal.
  ///
  /// If the run is wider than would fit in a row, it returns early and stops
  /// drawing.
  ///
  /// Returns the number of texels drawn.
  fn write(&mut self, start: Cell, texels: &[Texel]) -> io::Result<usize>;
}
impl dyn Tty {} // Object safe.

/// A coordinate in a [`Tty`]'s view.
///
/// Unlike Unix ttys, this function normalizes to zero indices to avoid
/// confusion.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Cell(usize, usize); // Col-row, zero-indexed.

impl Cell {
  /// Creates a new cell out of a zero-indexed column-row (i.e. `(x, y)`) pair.
  pub fn from_xy(col: usize, row: usize) -> Self {
    Self(col, row)
  }

  /// Creates a new cell out of one-indexed, half-word tty coordinates.
  fn from_tty(tty_coords: (u16, u16)) -> Self {
    Self(tty_coords.0 as usize, tty_coords.1 as usize)
  }

  /// Creates a new cell out of one-indexed, half-word tty coordinates.
  fn to_tty(self) -> (u16, u16) {
    debug_assert!(self.0 < u16::MAX as usize && self.1 < u16::MAX as usize);
    (self.0 as u16, self.1 as u16)
  }

  /// Returns this cell's row.
  pub fn row(self) -> usize {
    self.1
  }

  /// Returns this cell's column.
  pub fn col(self) -> usize {
    self.0
  }

  /// Returns the column-row (i.e. `(x, y)`) pair specifying this cell.
  pub fn xy(self) -> (usize, usize) {
    (self.0, self.1)
  }
}

/// A user-interaction event of some kind.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Event {
  /// A key press (we don't know if it's down or up).
  Key { key: Key, mods: BitFlags<Mod> },
  /// A mouse event.
  #[allow(missing_docs)]
  Mouse {
    button: Option<u8>,
    cell: Cell,
    action: MouseAction,
    mods: BitFlags<Mod>,
  },
  /// A change in the window size.
  Winch(Cell),
}

/// An action a user can take with a mouse.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum MouseAction {
  Press,
  Release,
  Drag,
  Move,
  ScrollUp,
  ScrollDown,
}

/// A key press that we understand.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
#[allow(missing_docs)]
pub enum Key {
  Glyph(char),
  Tab,
  BackTab,
  Enter,
  Backspace,

  Fn(u8),
  Delete,
  Insert,
  Esc,

  Left,
  Right,
  Up,
  Down,

  Home,
  End,
  PageUp,
  PageDown,
}

/// A key modifier.
#[enumflags2::bitflags]
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mod {
  Shift,
  Ctrl,
  Alt,
}

/// A [`Tty`] implemented over the stdin/stdout ANSI tty.
#[non_exhaustive]
#[derive(Default, Clone)]
pub struct AnsiTty {
  pub mouse_capture: bool,
}

impl AnsiTty {
  /// Installs a panic handler that reverses the alternate screen before
  /// anything is printed.
  pub fn install_panic_hook(&self) {
    let hook = panic::take_hook();
    let copy = self.clone();
    panic::set_hook(Box::new(move |info| {
      // Discard the error; we don't want a double panic.
      let _ = copy.clone().fini();
      hook(info);
    }));
  }
}

impl Tty for AnsiTty {
  fn init(&mut self) -> io::Result<()> {
    use crossterm::{cursor, event, execute, terminal};
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;
    if self.mouse_capture {
      execute!(io::stdout(), event::EnableMouseCapture)?;
    }
    Ok(())
  }

  fn fini(&mut self) -> io::Result<()> {
    use crossterm::{cursor, event, execute, terminal};
    terminal::disable_raw_mode()?;
    execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show)?;
    if self.mouse_capture {
      execute!(io::stdout(), event::DisableMouseCapture)?;
    }
    Ok(())
  }

  fn viewport(&mut self) -> io::Result<Cell> {
    let (x, y) = crossterm::terminal::size()?;
    Ok(Cell::from_xy(x as usize, y as usize)) // No need to normalize to 0-idx.
  }

  fn poll(&mut self, timeout: Option<Duration>) -> io::Result<Option<Event>> {
    #[allow(unused_imports)]
    use self::Event; // https://github.com/rust-lang/rust/issues/92904
    use crossterm::event::{self, Event as CtEvent, *};
    if timeout.is_some() && !event::poll(timeout.unwrap())? {
      return Ok(None);
    }

    match event::read()? {
      CtEvent::Key(KeyEvent { code, modifiers }) => {
        let key = match code {
          KeyCode::Char(c) => Key::Glyph(c),
          KeyCode::F(no) => Key::Fn(no),
          KeyCode::Backspace => Key::Backspace,
          KeyCode::Enter => Key::Enter,
          KeyCode::Left => Key::Left,
          KeyCode::Right => Key::Right,
          KeyCode::Up => Key::Up,
          KeyCode::Down => Key::Down,
          KeyCode::Home => Key::Home,
          KeyCode::End => Key::End,
          KeyCode::PageUp => Key::PageUp,
          KeyCode::PageDown => Key::PageDown,
          KeyCode::Tab => Key::Tab,
          KeyCode::BackTab => Key::BackTab,
          KeyCode::Delete => Key::Delete,
          KeyCode::Insert => Key::Insert,
          KeyCode::Esc => Key::Esc,
          _ => return Ok(None),
        };
        let mods = BitFlags::from_bits_truncate(modifiers.bits());
        Ok(Some(Event::Key { key, mods }))
      }
      CtEvent::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers,
      }) => {
        #[allow(unreachable_patterns)]
        let (action, button) = match kind {
          MouseEventKind::Down(b) => (MouseAction::Press, Some(b)),
          MouseEventKind::Up(b) => (MouseAction::Release, Some(b)),
          MouseEventKind::Drag(b) => (MouseAction::Drag, Some(b)),
          MouseEventKind::Moved => (MouseAction::Move, None),
          MouseEventKind::ScrollUp => (MouseAction::ScrollUp, None),
          MouseEventKind::ScrollDown => (MouseAction::ScrollDown, None),
          _ => return Ok(None),
        };
        let button = button.map(|b| match b {
          MouseButton::Left => 0,
          MouseButton::Right => 1,
          MouseButton::Middle => 2,
        });
        let cell = Cell::from_tty((column, row));
        let mods = BitFlags::from_bits_truncate(modifiers.bits());
        Ok(Some(Event::Mouse {
          button,
          action,
          cell,
          mods,
        }))
      }
      CtEvent::Resize(column, row) => {
        Ok(Some(Event::Winch(Cell::from_tty((column, row)))))
      }
    }
  }

  fn write(&mut self, start: Cell, texels: &[Texel]) -> io::Result<usize> {
    use crossterm::{cursor, execute, style};
    if texels.is_empty() {
      return Ok(0);
    }

    let (width, height) = self.viewport()?.xy();
    if start.col() >= width || start.row() >= height {
      return Ok(0);
    }

    let (x, y) = start.to_tty();

    execute!(io::stdout(), cursor::MoveTo(x, y))?;

    fn write_texel(
      (fg, bg, _): (
        Option<Option<Color>>,
        Option<Option<Color>>,
        Option<Weight>,
      ),
      c: Option<char>,
    ) -> io::Result<()> {
      let fg =
        fg.map(|c| c.map(Color::to_crossterm).unwrap_or(style::Color::Reset));
      let bg =
        bg.map(|c| c.map(Color::to_crossterm).unwrap_or(style::Color::Reset));
      execute!(
        io::stdout(),
        style::SetColors(style::Colors {
          foreground: fg,
          background: bg
        })
      )?;
      // TODO: Weights.
      write!(io::stdout(), "{}", c.unwrap_or(' '))
    }

    let (mut x, _) = start.xy();
    let mut prev = texels[0];
    write_texel(
      (Some(prev.fg()), Some(prev.bg()), Some(prev.weight())),
      prev.glyph(),
    )?;
    x += 1;

    for &texel in &texels[1..] {
      if x >= width {
        break;
      }

      write_texel(prev.style().diff(texel.style()), texel.glyph())?;
      prev = texel;
      x += 1;
    }

    io::stdout().flush()?;
    Ok(x - start.col())
  }
}

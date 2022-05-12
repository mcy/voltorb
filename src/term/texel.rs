/// "Terminal elements" or texels, analogous to a pixel or voxel.
use enumflags2::BitFlags;

/// A character color, representing the standard ANSI colors.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum Color {
  DkBlack, DkRed, DkGreen, DkYellow, DkBlue, DkMagenta, DkCyan, DkWhite,
  LtBlack, LtRed, LtGreen, LtYellow, LtBlue, LtMagenta, LtCyan, LtWhite,
}

impl Color {
  pub(crate) fn to_crossterm(self) -> crossterm::style::Color {
    use crossterm::style::Color;
    match self {
      Self::DkBlack => Color::Black,
      Self::DkRed => Color::DarkRed,
      Self::DkGreen => Color::DarkGreen,
      Self::DkYellow => Color::DarkYellow,
      Self::DkBlue => Color::DarkBlue,
      Self::DkMagenta => Color::DarkMagenta,
      Self::DkCyan => Color::DarkCyan,
      Self::DkWhite => Color::DarkGrey,
      Self::LtBlack => Color::Grey,
      Self::LtRed => Color::Red,
      Self::LtGreen => Color::Green,
      Self::LtYellow => Color::Yellow,
      Self::LtBlue => Color::Blue,
      Self::LtMagenta => Color::Magenta,
      Self::LtCyan => Color::Cyan,
      Self::LtWhite => Color::White,
    }
  }
}

/// A character weight, ranging from light to bold.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum Weight {
  Normal, Light, Bold,
}

#[enumflags2::bitflags]
#[repr(u16)]
#[derive(Copy, Clone, PartialEq, Debug)]
enum Meta {
  Bold = 1 << 0,
  Dim = 1 << 1,
  Uline = 1 << 2,

  BgReset = 1 << 8,
  FgReset = 1 << 9,
}

/// A texel style, including color and weight.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Style {
  fg: Color,
  bg: Color,
  meta: BitFlags<Meta>,
}

impl Style {
  /// Returns a new style with default settings.
  pub fn new() -> Self {
    Self {
      fg: Color::DkBlack,
      bg: Color::DkBlack,
      meta: Meta::FgReset | Meta::BgReset,
    }
  }

  /// Returns the foreground color.
  #[inline]
  pub fn fg(self) -> Option<Color> {
    if self.meta.contains(Meta::FgReset) {
      None
    } else {
      Some(self.fg)
    }
  }

  /// Returns a copy of this style with the given foreground color.
  #[inline]
  pub fn with_fg(mut self, color: impl Into<Option<Color>>) -> Self {
    self.meta.remove(Meta::FgReset);
    match color.into() {
      Some(rgb) => self.fg = rgb,
      None => self.meta |= Meta::FgReset,
    }
    self
  }

  /// Returns the background color.
  #[inline]
  pub fn bg(self) -> Option<Color> {
    if self.meta.contains(Meta::BgReset) {
      None
    } else {
      Some(self.bg)
    }
  }

  /// Returns a copy of this style with the given background color.
  #[inline]
  pub fn with_bg(mut self, color: impl Into<Option<Color>>) -> Self {
    self.meta.remove(Meta::BgReset);
    match color.into() {
      Some(rgb) => self.bg = rgb,
      None => self.meta |= Meta::BgReset,
    }
    self
  }

  /// Returns this style's weight.
  pub fn weight(self) -> Weight {
    if self.meta.contains(Meta::Bold) {
      Weight::Bold
    } else if self.meta.contains(Meta::Dim) {
      Weight::Light
    } else {
      Weight::Normal
    }
  }

  /// Returns a copy of this style with the given weight.
  #[inline]
  pub fn with_weight(mut self, weight: Weight) -> Self {
    self.meta.remove(Meta::Bold | Meta::Dim);
    match weight {
      Weight::Normal => {}
      Weight::Bold => self.meta |= Meta::Bold,
      Weight::Light => self.meta |= Meta::Dim,
    }
    self
  }

  /// Returns fg, bg, and weight that are different from self going to into.
  #[inline]
  pub(crate) fn diff(
    self,
    into: Self,
  ) -> (Option<Option<Color>>, Option<Option<Color>>, Option<Weight>) {
    let mut res = (None, None, None);
    if self.fg() != into.fg() {
      res.0 = Some(into.fg());
    }
    if self.bg() != into.bg() {
      res.1 = Some(into.bg());
    }
    if self.weight() != into.weight() {
      res.2 = Some(into.weight());
    }
    res
  }

  /// Returns an iterator over a series of texels built out of the characters
  /// in `s`, with this style applied.
  pub fn texels_from_str(self, s: &str) -> impl Iterator<Item = Texel> + '_ {
    s.chars().map(move |c| Texel::new(c).with_style(self))
  }
}

impl Default for Style {
  fn default() -> Self {
    Self::new()
  }
}

/*impl fmt::Display for Style {
  fn fmt(self, f: mut fmt::Formatter) -> fmt::Result {
    use termion::color::*;

    if self.meta.contains(Meta::FgReset) {
      write!(f, "{}", Fg(Reset))?;
    } else {
      self.fg.to_termion().write_fg(f)?;
    }

    if self.meta.contains(Meta::BgReset) {
      write!(f, "{}", Bg(Reset))?;
    } else {
      self.bg.to_termion().write_bg(f)?;
    }

    // TODO: line weight.

    Ok(())
  }
}*/

/// A "terminal element".
///
/// A texel consists of a "glyph" (a printable character), a foreground color,
/// and a background color; colors are optional.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Texel {
  glyph: Option<char>,
  style: Style,
}

impl Texel {
  /// Creates a new colorless texel with the given glyph.
  #[inline]
  pub fn new(glyph: char) -> Self {
    Self {
      glyph: Some(glyph),
      style: Style::new(),
    }
  }

  /// Creates a new colorless texel with no glyph.
  #[inline]
  pub fn empty() -> Self {
    Self {
      glyph: None,
      style: Style::new(),
    }
  }

  /// Returns this texel's glyph.
  #[inline]
  pub fn glyph(self) -> Option<char> {
    self.glyph
  }

  /// Returns a copy of this texel with the given glyph.
  #[inline]
  pub fn with_glyph(mut self, glyph: impl Into<Option<char>>) -> Self {
    self.glyph = glyph.into();
    self
  }

  /// Returns this texel's style.
  #[inline]
  pub fn style(self) -> Style {
    self.style
  }

  /// Returns a copy of this texel with the given style.
  #[inline]
  pub fn with_style(mut self, style: Style) -> Self {
    self.style = style;
    self
  }

  /// Returns this texel's foreground color.
  #[inline]
  pub fn fg(self) -> Option<Color> {
    self.style.fg()
  }

  /// Returns a copy of this texel with the given foreground color.
  #[inline]
  pub fn with_fg(mut self, color: impl Into<Option<Color>>) -> Self {
    self.style = self.style.with_fg(color.into());
    self
  }

  /// Returns this texel's background color.
  #[inline]
  pub fn bg(self) -> Option<Color> {
    self.style.bg()
  }

  /// Returns a copy of this texel with the given background color.
  #[inline]
  pub fn with_bg(mut self, color: impl Into<Option<Color>>) -> Self {
    self.style = self.style.with_bg(color.into());
    self
  }

  /// Returns this texel's weight.
  pub fn weight(self) -> Weight {
    self.style.weight()
  }

  /// Returns a copy of this texel with the given weight.
  #[inline]
  pub fn with_weight(mut self, weight: Weight) -> Self {
    self.style = self.style.with_weight(weight);
    self
  }
}

// Extension trait for creating texels from `char`s.
pub trait FromChar {
  fn with_style(self, style: Style) -> Texel;
}

impl<C: Into<char>> FromChar for C {
  fn with_style(self, style: Style) -> Texel {
    Texel::new(self.into()).with_style(style)
  }
}

/*impl fmt::Display for Texel {
  fn fmt(self, f: mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}{}", self.style, self.glyph.unwrap_or(' '))
  }
}*/

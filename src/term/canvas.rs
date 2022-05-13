//! A texel compositor.

use std::borrow::Cow;
use std::io;
use std::iter;
use std::mem;

use crate::term::texel::Texel;
use crate::term::Cell;
use crate::term::Tty;

/// A layer of texels to draw as part of a rendering operation on a [`Canvas`].
pub struct Layer<'a> {
  /// The cell to place the upper-left corner of the layer at.
  pub origin: Cell,
  /// The width of a row in `data`, used for computing its dimensions.
  pub stride: usize,
  /// The texel data to render.
  pub data: Cow<'a, [Texel]>,
}

impl Layer<'_> {
  /// Returns the coordinates of the opposite corner relative to `origin`,
  /// inclusive.
  pub fn antipode(&self) -> Cell {
    Cell::from_xy(
      self.origin.col() + self.stride - 1,
      self.origin.row() + self.data.len() / self.stride - 1,
    )
  }

  /// Computes the bounding box for all layers in `layers`, inclusive.
  pub fn bounding_box<'a>(
    layers: impl IntoIterator<Item = &'a Layer<'a>>,
  ) -> (Cell, Cell) {
    let mut upper = Cell::from_xy(usize::MAX, usize::MAX);
    let mut lower = Cell::from_xy(0, 0);
    for layer in layers {
      upper = Cell::from_xy(
        upper.col().min(layer.origin.col()),
        upper.row().min(layer.origin.row()),
      );
      lower = Cell::from_xy(
        lower.col().max(layer.antipode().col()),
        lower.row().max(layer.antipode().row()),
      );
    }
    (upper, lower)
  }
}

/// A texel compositor.
///
/// A `Canvas` is a texel buffer for compositing and rendering images to a
/// [`Tty`] in a way that avoids completely thrashing it. It is generally a bad
/// idea to print every line in the terminal just to update a few texels.
///
/// A `Canvas` renders a stack of [`Layers`], which are simply blocks of
/// texels drawn at a specific point on the terminal. They are composited first
/// onto a memory buffer, and then only the parts that have changed relative to
/// what's on the screen are drawn.
pub struct Canvas {
  viewport: Cell,
  buffer: Vec<Texel>,
}

impl Canvas {
  /// Creates a new `Canvas` for the viewport of the given size.
  pub fn new(viewport: Cell) -> Self {
    let (x, y) = viewport.xy();
    let buffer = Vec::with_capacity(x * y);
    Self { viewport, buffer }
  }

  /// Returns the current viewport size for the `Canvas`.
  pub fn viewport(&self) -> Cell {
    self.viewport
  }

  /// Applies a resize operation.
  ///
  /// This allocates (if necessary) a new buffer, discarding the render cache.
  pub fn winch(&mut self, viewport: Cell) {
    self.viewport = viewport;
    self.buffer.clear();

    let (x, y) = self.viewport.xy();
    let new_cap = x * y;
    if new_cap < self.buffer.capacity() / 2 {
      self.buffer.shrink_to(new_cap);
    } else if let Some(growth) = new_cap.checked_sub(self.buffer.capacity()) {
      self.buffer.reserve(growth);
    }
  }

  /// Renders `layers` onto `tty`.
  ///
  /// This function is not intended to be called on multiple different `tty`s,
  /// since it remembers what was written the *last* time this function was
  /// called.
  pub fn render<'a>(
    &mut self,
    layers: impl IntoIterator<Item = Layer<'a>>,
    tty: &mut dyn Tty,
  ) -> io::Result<()> {
    let mut side_buffer;
    let (buffer, old) = if self.buffer.is_empty() {
      (&mut self.buffer, None)
    } else {
      side_buffer = Vec::with_capacity(self.buffer.capacity());
      (&mut side_buffer, Some(&mut self.buffer))
    };

    buffer.extend(iter::repeat(Texel::empty()).take(buffer.capacity()));
    for l in layers {
      let (x, _) = self.viewport.xy();
      let (ox, oy) = l.origin.xy();
      if ox >= x {
        continue;
      }

      let src_iter = l.data.chunks(l.stride);
      let dst_iter = buffer.chunks_mut(x).skip(oy);
      for (dst, src) in Iterator::zip(dst_iter, src_iter) {
        for (dst, src) in Iterator::zip(dst[ox..].iter_mut(), src) {
          if src.glyph().is_some() {
            *dst = *src;
          }
        }
      }
    }

    for (i, line) in buffer.chunks(self.viewport.col()).enumerate() {
      if let Some(old) = &old {
        let mut last_boundary = 0;
        let mut same_at_last_boundary = true;
        for (j, &tx) in line.iter().enumerate() {
          let old = old[i * self.viewport.col() + j];
          let same = old == tx;
          if same == same_at_last_boundary {
            continue;
          }
          same_at_last_boundary = same;

          if same {
            let start = Cell::from_xy(last_boundary, i);
            tty.write(start, &line[last_boundary..j])?;
          }
          last_boundary = j;
        }
        if same_at_last_boundary {
          let start = Cell::from_xy(last_boundary, i);
          tty.write(start, &line[last_boundary..])?;
        }
      } else {
        let start = Cell::from_xy(0, i);
        tty.write(start, line)?;
      }
    }

    self.buffer = mem::take(buffer);
    Ok(())
  }
}

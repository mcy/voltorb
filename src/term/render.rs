//! Simple TUI rendering engine.

use std::borrow::Cow;
use std::io;
use std::iter;
use std::mem;

use crate::term::texel::Texel;
use crate::term::tty::Cell;
use crate::term::tty::Tty;

pub struct Canvas<'tty> {
  tty: &'tty mut dyn Tty,
  viewport: Cell,
  buffer: Vec<Texel>,
}

pub struct Layer<'a> {
  pub origin: Cell,
  pub stride: usize,
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
  pub fn bounding_box<'b>(
    layers: impl IntoIterator<Item = &'b Layer<'b>>,
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

impl<'tty> Canvas<'tty> {
  pub fn new(tty: &'tty mut dyn Tty) -> io::Result<Self> {
    tty.init()?;
    let viewport = tty.viewport()?;
    let (x, y) = viewport.xy();
    let buffer = Vec::with_capacity(x * y);
    Ok(Self {
      tty,
      viewport,
      buffer,
    })
  }

  pub fn viewport(&self) -> Cell {
    self.viewport
  }

  pub fn tty(&mut self) -> &mut dyn Tty {
    self.tty
  }

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

  pub fn render<'a>(
    &mut self,
    layers: impl IntoIterator<Item = Layer<'a>>,
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
            self.tty.write(start, &line[last_boundary..j])?;
          }
          last_boundary = j;
        }
        if same_at_last_boundary {
          let start = Cell::from_xy(last_boundary, i);
          self.tty.write(start, &line[last_boundary..])?;
        }
      } else {
        let start = Cell::from_xy(0, i);
        self.tty.write(start, line)?;
      }
    }

    self.buffer = mem::take(buffer);
    Ok(())
  }
}
